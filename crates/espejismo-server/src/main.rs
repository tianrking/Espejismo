use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::Parser;
use espejismo_core::config::{encode_config_base64, example_config};
use espejismo_core::{
    accept_handshake_with_replay, idle_copy_bidirectional, init_logging, load_config, parse_psk,
    read_tunnel_request, spawn_admin_server, spawn_frame_transport, AdminState, ConfigInput,
    EgressPolicy, EspejismoConfig, FrameOptions, HandshakeConfig, LogConfig, LogFormat, Metrics,
    ProbeDefenseMode, ReplayCache, TunnelRequest,
};
use futures::StreamExt;
use rand::seq::SliceRandom;
use rand::Rng;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{lookup_host, TcpListener, TcpStream, UdpSocket};
use tokio::sync::Semaphore;
use tokio::time::{sleep, timeout, Duration};
use tokio_yamux::{Config as YamuxConfig, Session, StreamHandle};
use tracing::{debug, info};

mod tarpit;

#[derive(Parser, Debug)]
#[command(name = "espejismo-remote")]
struct Args {
    #[arg(long)]
    config: Option<String>,
    #[arg(long)]
    config_base64: Option<String>,
    #[arg(long)]
    print_example_config: bool,
    #[arg(long)]
    print_example_config_base64: bool,
    #[arg(long)]
    listen: Option<SocketAddr>,
    #[arg(long, env = "ESPEJISMO_PSK")]
    psk: Option<String>,
    #[arg(long)]
    clock_skew_secs: Option<i64>,
    #[arg(long)]
    max_padding: Option<usize>,
    #[arg(long)]
    jitter_ms: Option<u64>,
    #[arg(long)]
    padding_chance_percent: Option<u8>,
    #[arg(long)]
    backpressure_threshold_ms: Option<u64>,
    #[arg(long)]
    backpressure_cooldown_ms: Option<u64>,
    #[arg(long)]
    handshake_timeout_ms: Option<u64>,
    #[arg(long)]
    reject_delay_ms: Option<u64>,
    #[arg(long)]
    max_handshake_padding: Option<usize>,
    #[arg(long)]
    replay_window_secs: Option<i64>,
    #[arg(long)]
    puzzle_bits: Option<u8>,
    #[arg(long)]
    tunnel_buffer: Option<usize>,
    #[arg(long)]
    cold_start_delay_ms: Option<u64>,
    #[arg(long)]
    tarpit_max: Option<usize>,
    #[arg(long)]
    tarpit_hold_secs: Option<u64>,
    #[arg(long)]
    log_level: Option<String>,
    #[arg(long)]
    log_format: Option<String>,
    #[arg(long)]
    log_file: Option<PathBuf>,
    #[arg(long)]
    no_log_ansi: bool,
    #[arg(long)]
    admin_listen: Option<SocketAddr>,
    #[arg(long)]
    admin_token: Option<String>,
}

#[derive(Clone)]
struct RemoteRuntime {
    listen: SocketAddr,
    handshake: HandshakeConfig,
    frames: FrameOptions,
    handshake_timeout: Duration,
    reject_delay: Duration,
    replay_window_secs: i64,
    tunnel_buffer: usize,
    cold_start_delay: Duration,
    tarpit_max: usize,
    tarpit_hold: Duration,
    fallback_http: FallbackHttpRuntime,
    admin_listen: Option<SocketAddr>,
    admin_token: Option<String>,
    egress: EgressPolicy,
    idle_timeout: Duration,
    max_streams: u32,
}

#[derive(Clone)]
struct FallbackHttpRuntime {
    enabled: bool,
    upstream: Option<String>,
    probe_timeout: Duration,
    server: String,
    body: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.print_example_config || args.print_example_config_base64 {
        let example = example_config();
        if args.print_example_config_base64 {
            println!("{}", encode_config_base64(&example));
        } else {
            print!("{example}");
        }
        return Ok(());
    }

    let mut config = load_config(ConfigInput {
        path: args.config.clone(),
        base64: args.config_base64.clone(),
    })?;
    apply_log_overrides(&mut config.logging, &args)?;
    let _log_guard = init_logging(&config.logging)?;
    let runtime = build_runtime(config, &args)?;
    let metrics = Metrics::default();
    if let Some(addr) = runtime.admin_listen {
        spawn_admin_server(
            addr,
            AdminState {
                role: "remote".to_string(),
                metrics: metrics.clone(),
                token: runtime.admin_token.clone(),
            },
        );
    }

    let listener = TcpListener::bind(runtime.listen).await?;
    let tarpit = tarpit::TarpitManager::spawn(runtime.tarpit_max, runtime.tarpit_hold);
    let replay = Arc::new(tokio::sync::Mutex::new(ReplayCache::new(
        runtime.replay_window_secs,
    )));
    info!(listen = %runtime.listen, "remote listening with yamux tunnel support");

    loop {
        let (socket, peer) = listener.accept().await?;
        let replay = replay.clone();
        let runtime = runtime.clone();
        let tarpit = tarpit.clone();
        let metrics = metrics.clone();
        metrics.inc_accepted();
        tokio::spawn(async move {
            if let Err(err) = handle_peer(socket, runtime, replay, tarpit, metrics).await {
                debug!(%peer, error = %err, "remote peer ended");
            }
        });
    }
}

fn apply_log_overrides(config: &mut LogConfig, args: &Args) -> Result<()> {
    if let Some(level) = &args.log_level {
        config.level = level.clone();
    }
    if let Some(format) = &args.log_format {
        config.format = parse_log_format(format)?;
    }
    if let Some(file) = &args.log_file {
        config.file = Some(file.clone());
    }
    if args.no_log_ansi {
        config.ansi = false;
    }
    Ok(())
}

fn parse_log_format(format: &str) -> Result<LogFormat> {
    match format {
        "compact" => Ok(LogFormat::Compact),
        "pretty" => Ok(LogFormat::Pretty),
        "json" => Ok(LogFormat::Json),
        _ => anyhow::bail!("log format must be compact, pretty, or json"),
    }
}

fn build_runtime(config: EspejismoConfig, args: &Args) -> Result<RemoteRuntime> {
    let psk = args
        .psk
        .clone()
        .or(config.shared.psk)
        .context("provide psk in config, --psk, or ESPEJISMO_PSK")?;
    let stealth_frame_size = config.shared.stealth.frame_size;
    let stealth_tick_ms = config.shared.stealth.tick_ms;
    let obfuscation_profile = config.shared.obfuscation.profile;
    let stealth_handshake = obfuscation_profile
        .is_stealth()
        .then_some(stealth_frame_size);

    Ok(RemoteRuntime {
        listen: args.listen.unwrap_or(config.remote.listen),
        handshake: HandshakeConfig::new(
            parse_psk(&psk)?,
            args.clock_skew_secs
                .unwrap_or(config.shared.clock_skew_secs),
            args.max_handshake_padding
                .unwrap_or(config.remote.max_handshake_padding),
            args.puzzle_bits.unwrap_or(config.shared.puzzle_bits),
        )
        .with_stealth_frame_size(stealth_handshake),
        frames: FrameOptions {
            max_padding: args.max_padding.unwrap_or(config.shared.max_padding),
            jitter_ms: args.jitter_ms.unwrap_or(config.shared.jitter_ms),
            padding_chance_percent: args
                .padding_chance_percent
                .unwrap_or(config.shared.padding_chance_percent),
            backpressure_threshold_ms: args
                .backpressure_threshold_ms
                .unwrap_or(config.shared.backpressure_threshold_ms),
            backpressure_cooldown_ms: args
                .backpressure_cooldown_ms
                .unwrap_or(config.shared.backpressure_cooldown_ms),
            obfuscation_profile,
            randomize_chunks: config.shared.obfuscation.randomize_chunks,
            min_chunk: config.shared.obfuscation.min_chunk,
            max_chunk: config.shared.obfuscation.max_chunk,
            stealth_frame_size,
            stealth_tick_ms,
        },
        handshake_timeout: Duration::from_millis(
            args.handshake_timeout_ms
                .unwrap_or(config.remote.handshake_timeout_ms),
        ),
        reject_delay: Duration::from_millis(
            args.reject_delay_ms
                .unwrap_or(config.remote.reject_delay_ms)
                .min(10_000),
        ),
        replay_window_secs: args
            .replay_window_secs
            .unwrap_or(config.remote.replay_window_secs),
        tunnel_buffer: args.tunnel_buffer.unwrap_or(config.shared.tunnel_buffer),
        cold_start_delay: Duration::from_millis(
            args.cold_start_delay_ms
                .unwrap_or(config.remote.cold_start_delay_ms),
        ),
        tarpit_max: args.tarpit_max.unwrap_or(config.remote.tarpit_max),
        tarpit_hold: Duration::from_secs(
            args.tarpit_hold_secs
                .unwrap_or(config.remote.tarpit_hold_secs),
        ),
        fallback_http: FallbackHttpRuntime {
            enabled: matches!(
                config.remote.fallback_http.mode,
                ProbeDefenseMode::HttpFallback
            ) || config.remote.fallback_http.enabled,
            upstream: config.remote.fallback_http.upstream,
            probe_timeout: Duration::from_millis(config.remote.fallback_http.probe_timeout_ms),
            server: config.remote.fallback_http.server,
            body: config.remote.fallback_http.body,
        },
        admin_listen: args.admin_listen.or(config.admin.listen),
        admin_token: args.admin_token.clone().or(config.admin.token),
        egress: config.remote.egress.into(),
        idle_timeout: Duration::from_secs(config.shared.idle_timeout_secs),
        max_streams: config.shared.max_streams,
    })
}

async fn handle_peer(
    mut inbound: TcpStream,
    runtime: RemoteRuntime,
    replay: Arc<tokio::sync::Mutex<ReplayCache>>,
    tarpit: tarpit::TarpitManager,
    metrics: Metrics,
) -> Result<()> {
    if should_route_to_http_fallback(&mut inbound, &runtime.fallback_http).await? {
        route_http_fallback(inbound, &runtime.fallback_http).await?;
        return Ok(());
    }

    metrics.inc_active_physical();
    let keys = match timeout(
        runtime.handshake_timeout,
        accept_handshake_with_replay(&mut inbound, &runtime.handshake, replay),
    )
    .await
    {
        Ok(Ok(keys)) => {
            metrics.inc_handshake_success();
            keys
        }
        Ok(Err(err)) => {
            metrics.inc_handshake_failure();
            metrics.dec_active_physical();
            fallback_or_reject(
                inbound,
                &runtime.fallback_http,
                runtime.reject_delay,
                &tarpit,
            )
            .await;
            return Err(err);
        }
        Err(err) => {
            metrics.inc_handshake_failure();
            metrics.dec_active_physical();
            fallback_or_reject(
                inbound,
                &runtime.fallback_http,
                runtime.reject_delay,
                &tarpit,
            )
            .await;
            return Err(err.into());
        }
    };

    if !runtime.cold_start_delay.is_zero() {
        sleep(runtime.cold_start_delay).await;
    }

    let transport = spawn_frame_transport(inbound, keys, runtime.frames, runtime.tunnel_buffer);
    let mut session = Session::new_server(transport, YamuxConfig::default());
    let stream_limit = Arc::new(Semaphore::new(runtime.max_streams as usize));
    while let Some(stream) = session.next().await {
        let stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                metrics.dec_active_physical();
                return Err(err.into());
            }
        };

        if runtime.max_streams == 0 {
            debug!(
                max = runtime.max_streams,
                "rejecting stream: stream limit disabled"
            );
            continue;
        }

        let stream_permit = stream_limit
            .clone()
            .acquire_owned()
            .await
            .map_err(|err| anyhow::anyhow!("stream limit closed: {err}"))?;
        let metrics = metrics.clone();
        let egress = runtime.egress.clone();
        let idle = runtime.idle_timeout;
        tokio::spawn(async move {
            let _stream_permit = stream_permit;
            if let Err(err) = handle_mux_stream(stream, metrics, egress, idle).await {
                debug!(error = %err, "mux stream ended");
            }
        });
    }
    metrics.dec_active_physical();
    Ok(())
}

async fn handle_mux_stream(
    mut stream: StreamHandle,
    metrics: Metrics,
    egress: EgressPolicy,
    idle: Duration,
) -> Result<()> {
    metrics.inc_active_stream();
    metrics.inc_stream_opened();
    let result = handle_mux_stream_inner(&mut stream, metrics.clone(), egress, idle).await;
    if result.is_err() {
        metrics.inc_stream_failed();
    }
    metrics.dec_active_stream();
    result
}

async fn handle_mux_stream_inner(
    stream: &mut StreamHandle,
    metrics: Metrics,
    egress: EgressPolicy,
    idle: Duration,
) -> Result<()> {
    match read_tunnel_request(stream).await? {
        TunnelRequest::TcpConnect { authority } => {
            let mut remote = connect_egress_tcp(&authority, &egress).await?;
            info!(target = %authority, "mux TCP relay opened");
            let (client_to_remote, remote_to_client) =
                idle_copy_bidirectional(stream, &mut remote, idle).await?;
            metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
        }
        TunnelRequest::UdpDatagram { authority, payload } => {
            let response = relay_udp_datagram(&authority, &payload, &egress, idle).await?;
            metrics.add_tunnel_bytes(payload.len() as u64, response.len() as u64);
            anyhow::ensure!(
                response.len() <= u16::MAX as usize,
                "UDP response too large"
            );
            stream.write_u16(response.len() as u16).await?;
            stream.write_all(&response).await?;
            stream.shutdown().await?;
        }
    }
    Ok(())
}

async fn connect_egress_tcp(authority: &str, egress: &EgressPolicy) -> Result<TcpStream> {
    egress
        .validate_authority(authority)
        .context("egress policy rejected target")?;
    if let Some(proxy) = &egress.socks5_proxy {
        return connect_via_socks5_proxy(proxy, authority).await;
    }
    let mut last_error = None;
    for addr in lookup_host(authority)
        .await
        .with_context(|| format!("resolve {authority}"))?
    {
        if let Err(err) = egress.validate_resolved_addr(addr) {
            last_error = Some(err);
            continue;
        }
        match TcpStream::connect(addr).await {
            Ok(stream) => return Ok(stream),
            Err(err) => last_error = Some(err.into()),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no resolved egress address")))
        .with_context(|| format!("connect {authority}"))
}

async fn connect_via_socks5_proxy(proxy: &str, authority: &str) -> Result<TcpStream> {
    let (host, port) = espejismo_core::split_authority(authority)?;
    let mut stream = TcpStream::connect(proxy)
        .await
        .with_context(|| format!("connect SOCKS5 proxy {proxy}"))?;
    stream.write_all(&[0x05, 0x01, 0x00]).await?;
    let mut method = [0_u8; 2];
    stream.read_exact(&mut method).await?;
    anyhow::ensure!(
        method == [0x05, 0x00],
        "SOCKS5 proxy rejected no-auth method"
    );
    let host_bytes = host.as_bytes();
    anyhow::ensure!(
        host_bytes.len() <= u8::MAX as usize,
        "SOCKS5 proxy target host too long"
    );
    let mut request = vec![0x05, 0x01, 0x00, 0x03, host_bytes.len() as u8];
    request.extend_from_slice(host_bytes);
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).await?;
    let mut head = [0_u8; 4];
    stream.read_exact(&mut head).await?;
    anyhow::ensure!(
        head[0] == 0x05 && head[1] == 0x00,
        "SOCKS5 proxy CONNECT failed"
    );
    match head[3] {
        0x01 => {
            let mut skip = [0_u8; 6];
            stream.read_exact(&mut skip).await?;
        }
        0x03 => {
            let len = stream.read_u8().await? as usize;
            let mut skip = vec![0_u8; len + 2];
            stream.read_exact(&mut skip).await?;
        }
        0x04 => {
            let mut skip = [0_u8; 18];
            stream.read_exact(&mut skip).await?;
        }
        atyp => anyhow::bail!("SOCKS5 proxy returned unsupported address type {atyp}"),
    }
    Ok(stream)
}

async fn relay_udp_datagram(
    authority: &str,
    payload: &[u8],
    egress: &EgressPolicy,
    idle: Duration,
) -> Result<Vec<u8>> {
    egress
        .validate_authority(authority)
        .context("egress policy rejected UDP target")?;
    let mut selected = None;
    for addr in lookup_host(authority)
        .await
        .with_context(|| format!("resolve {authority}"))?
    {
        if egress.validate_resolved_addr(addr).is_ok() {
            selected = Some(addr);
            break;
        }
    }
    let target = selected.context("no allowed UDP egress address")?;
    let bind = if target.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind).await?;
    socket.connect(target).await?;
    socket.send(payload).await?;
    let mut response = vec![0_u8; 65_535];
    let n = timeout(
        idle.min(Duration::from_secs(10)),
        socket.recv(&mut response),
    )
    .await??;
    response.truncate(n);
    Ok(response)
}

async fn reject_or_quarantine(
    stream: TcpStream,
    reject_delay: Duration,
    tarpit: &tarpit::TarpitManager,
) {
    if reject_delay.is_zero() {
        tarpit.quarantine(stream).await;
    } else {
        quiet_reject(stream, reject_delay).await;
    }
}

async fn fallback_or_reject(
    mut stream: TcpStream,
    fallback: &FallbackHttpRuntime,
    reject_delay: Duration,
    tarpit: &tarpit::TarpitManager,
) {
    if fallback.enabled {
        let _ = write_builtin_fallback_response(&mut stream, fallback).await;
        return;
    }
    reject_or_quarantine(stream, reject_delay, tarpit).await;
}

async fn quiet_reject(mut stream: TcpStream, delay: Duration) {
    if !delay.is_zero() {
        sleep(delay).await;
    }
    let _ = stream.shutdown().await;
}

async fn should_route_to_http_fallback(
    stream: &mut TcpStream,
    fallback: &FallbackHttpRuntime,
) -> Result<bool> {
    if !fallback.enabled {
        return Ok(false);
    }
    let mut buf = [0_u8; 16];
    let n = match timeout(fallback.probe_timeout, stream.peek(&mut buf)).await {
        Ok(Ok(n)) => n,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => return Ok(false),
    };
    if n == 0 {
        return Ok(false);
    }
    Ok(looks_like_http_probe(&buf[..n]))
}

fn looks_like_http_probe(prefix: &[u8]) -> bool {
    let methods: [&[u8]; 10] = [
        b"GET ",
        b"POST ",
        b"HEAD ",
        b"PUT ",
        b"PATCH ",
        b"DELETE ",
        b"OPTIONS ",
        b"CONNECT ",
        b"TRACE ",
        b"PRI * HTTP/2.0",
    ];
    methods.iter().any(|m| prefix.starts_with(m))
}

async fn route_http_fallback(mut inbound: TcpStream, fallback: &FallbackHttpRuntime) -> Result<()> {
    if let Some(upstream) = &fallback.upstream {
        let mut upstream_stream = TcpStream::connect(upstream)
            .await
            .with_context(|| format!("connect fallback upstream {upstream}"))?;
        let _ = copy_bidirectional(&mut inbound, &mut upstream_stream).await?;
        return Ok(());
    }
    write_builtin_fallback_response(&mut inbound, fallback).await
}

async fn write_builtin_fallback_response(
    stream: &mut TcpStream,
    fallback: &FallbackHttpRuntime,
) -> Result<()> {
    let response = build_builtin_fallback_response(fallback);
    stream.write_all(&response).await?;
    stream.shutdown().await?;
    Ok(())
}

fn build_builtin_fallback_response(fallback: &FallbackHttpRuntime) -> Vec<u8> {
    let body = fallback.body.as_bytes();
    let now = SystemTime::now();
    let modified = now
        .checked_sub(Duration::from_secs(
            rand::thread_rng().gen_range(600..=86_400),
        ))
        .unwrap_or(UNIX_EPOCH);
    let etag = format!(
        "\"{:x}-{:x}-{:x}\"",
        body.len(),
        unix_secs(modified),
        rand::thread_rng().gen::<u32>()
    );
    let cache_header = [
        "Cache-Control: no-cache\r\n",
        "Cache-Control: max-age=0\r\n",
        "Accept-Ranges: bytes\r\n",
    ]
    .choose(&mut rand::thread_rng())
    .copied()
    .unwrap_or("Accept-Ranges: bytes\r\n");

    format!(
        "HTTP/1.1 200 OK\r\nDate: {}\r\nServer: {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nLast-Modified: {}\r\nETag: {}\r\n{}Connection: close\r\n\r\n{}",
        http_date(now),
        fallback_server_header(&fallback.server),
        body.len(),
        http_date(modified),
        etag,
        cache_header,
        fallback.body
    )
    .into_bytes()
}

fn fallback_server_header(configured: &str) -> String {
    let trimmed = configured.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("nginx") {
        let versions = ["1.18.0", "1.20.2", "1.22.1", "1.24.0", "1.25.5"];
        let version = versions
            .choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or("1.24.0");
        return format!("nginx/{version}");
    }
    if trimmed.eq_ignore_ascii_case("caddy") {
        return "Caddy".to_string();
    }
    trimmed.to_string()
}

fn http_date(time: SystemTime) -> String {
    let secs = unix_secs(time) as i64;
    let days = secs.div_euclid(86_400);
    let day_secs = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;
    let weekdays = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let weekday = weekdays[days.rem_euclid(7) as usize];
    let month_name = months[(month - 1) as usize];
    format!("{weekday}, {day:02} {month_name} {year:04} {hour:02}:{minute:02}:{second:02} GMT")
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

fn unix_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{build_builtin_fallback_response, looks_like_http_probe, FallbackHttpRuntime};
    use tokio::time::Duration;

    #[test]
    fn detects_common_http_methods() {
        assert!(looks_like_http_probe(b"GET / HTTP/1.1\r\n"));
        assert!(looks_like_http_probe(b"POST /submit HTTP/1.1\r\n"));
        assert!(looks_like_http_probe(
            b"CONNECT example.com:443 HTTP/1.1\r\n"
        ));
    }

    #[test]
    fn ignores_non_http_prefixes() {
        assert!(!looks_like_http_probe(b"\x16\x03\x01\x02\x00"));
        assert!(!looks_like_http_probe(b"\x8f\xf2\x00\x11"));
    }

    #[test]
    fn builtin_fallback_response_has_browser_like_headers() {
        let fallback = FallbackHttpRuntime {
            enabled: true,
            upstream: None,
            probe_timeout: Duration::from_millis(250),
            server: "nginx".to_string(),
            body: "<html>ok</html>".to_string(),
        };

        let response = String::from_utf8(build_builtin_fallback_response(&fallback)).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("\r\nDate: "));
        assert!(response.contains("\r\nServer: nginx/"));
        assert!(response.contains("\r\nLast-Modified: "));
        assert!(response.contains("\r\nETag: "));
        assert!(response.contains("\r\nContent-Length: 15\r\n"));
    }
}
