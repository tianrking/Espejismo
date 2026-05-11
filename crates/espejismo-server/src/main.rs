use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::Parser;
use espejismo_core::config::{encode_config_base64, example_config};
use espejismo_core::{
    accept_handshake_with_users, check_for_update, init_logging, load_config, parse_config,
    parse_psk, read_tunnel_request, spawn_admin_server, spawn_frame_transport, AdminAction,
    AdminState, ConfigInput, EgressPolicy, EspejismoConfig, FrameOptions, HandshakeConfig,
    HandshakeUser, LogConfig, LogFormat, Metrics, ProbeDefenseMode, ReplayCache, TunnelRequest,
};
use futures::StreamExt;
use rand::seq::SliceRandom;
use rand::Rng;
use serde_json::json;
use tokio::io::{copy_bidirectional, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{lookup_host, TcpListener, TcpStream, UdpSocket};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{sleep, timeout, Duration};
use tokio_yamux::{Config as YamuxConfig, Session, StreamHandle};
use tracing::{debug, info};

mod limits;
mod tarpit;

use limits::{UserLimitConfig, UserLimitRegistry};

#[derive(Parser, Debug, Clone)]
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
    check_update: bool,
    #[arg(long)]
    update_url: Option<String>,
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
    settings: Arc<RwLock<RemoteSettings>>,
    replay_window_secs: i64,
    tunnel_buffer: usize,
    tarpit_max: usize,
    tarpit_hold: Duration,
    admin_listen: Option<SocketAddr>,
    admin_token: Option<String>,
    reload_source: Option<ConfigInput>,
    reload_args: Args,
}

#[derive(Clone)]
struct RemoteSettings {
    users: Arc<Vec<HandshakeUser>>,
    frames: FrameOptions,
    handshake_timeout: Duration,
    reject_delay: Duration,
    cold_start_delay: Duration,
    fallback_http: FallbackHttpRuntime,
    egress: EgressPolicy,
    idle_timeout: Duration,
    max_streams: u32,
    limits: UserLimitRegistry,
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
    if args.check_update {
        print_update_check(args.update_url.as_deref())?;
        return Ok(());
    }
    if args.print_example_config || args.print_example_config_base64 {
        let example = example_config();
        if args.print_example_config_base64 {
            println!("{}", encode_config_base64(&example));
        } else {
            print!("{example}");
        }
        return Ok(());
    }

    let config_input = ConfigInput {
        path: args.config.clone(),
        base64: args.config_base64.clone(),
    };
    let mut config = load_config(config_input.clone())?;
    apply_log_overrides(&mut config.logging, &args)?;
    let _log_guard = init_logging(&config.logging)?;
    let runtime = build_runtime(config, &args, config_input)?;
    let metrics = Metrics::default();
    if let Some(addr) = runtime.admin_listen {
        let reload = runtime.reload_action();
        spawn_admin_server(
            addr,
            AdminState {
                role: "remote".to_string(),
                metrics: metrics.clone(),
                token: runtime.admin_token.clone(),
                reload,
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

fn print_update_check(update_url: Option<&str>) -> Result<()> {
    let info = check_for_update(env!("CARGO_PKG_VERSION"), update_url)?;
    if info.update_available {
        println!(
            "update available: {} -> {}",
            info.current_version, info.latest_version
        );
        if let Some(url) = info.release_url {
            println!("release: {url}");
        }
    } else {
        println!("up to date: {}", info.current_version);
    }
    Ok(())
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

fn build_handshake_users(
    config: &EspejismoConfig,
    args: &Args,
    stealth_handshake: Option<usize>,
) -> Result<Vec<HandshakeUser>> {
    let mut users = Vec::new();
    if !config.remote.users.is_empty() {
        for user in &config.remote.users {
            users.push(HandshakeUser {
                name: user.name.clone(),
                config: HandshakeConfig::new(
                    parse_psk(&user.psk)?,
                    args.clock_skew_secs
                        .unwrap_or(config.shared.clock_skew_secs),
                    args.max_handshake_padding
                        .unwrap_or(config.remote.max_handshake_padding),
                    args.puzzle_bits.unwrap_or(config.shared.puzzle_bits),
                )
                .with_stealth_frame_size(stealth_handshake),
            });
        }
        return Ok(users);
    }

    let psk = args
        .psk
        .clone()
        .or_else(|| config.shared.psk.clone())
        .context("provide shared.psk, remote.users, --psk, or ESPEJISMO_PSK")?;
    users.push(HandshakeUser {
        name: "default".to_string(),
        config: HandshakeConfig::new(
            parse_psk(&psk)?,
            args.clock_skew_secs
                .unwrap_or(config.shared.clock_skew_secs),
            args.max_handshake_padding
                .unwrap_or(config.remote.max_handshake_padding),
            args.puzzle_bits.unwrap_or(config.shared.puzzle_bits),
        )
        .with_stealth_frame_size(stealth_handshake),
    });
    Ok(users)
}

fn build_runtime(
    config: EspejismoConfig,
    args: &Args,
    reload_source: ConfigInput,
) -> Result<RemoteRuntime> {
    let settings = build_remote_settings(&config, args)?;
    Ok(RemoteRuntime {
        listen: args.listen.unwrap_or(config.remote.listen),
        settings: Arc::new(RwLock::new(settings)),
        replay_window_secs: args
            .replay_window_secs
            .unwrap_or(config.remote.replay_window_secs),
        tunnel_buffer: args.tunnel_buffer.unwrap_or(config.shared.tunnel_buffer),
        tarpit_max: args.tarpit_max.unwrap_or(config.remote.tarpit_max),
        tarpit_hold: Duration::from_secs(
            args.tarpit_hold_secs
                .unwrap_or(config.remote.tarpit_hold_secs),
        ),
        admin_listen: args.admin_listen.or(config.admin.listen),
        admin_token: args.admin_token.clone().or(config.admin.token),
        reload_source: (reload_source.path.is_some() || reload_source.base64.is_some())
            .then_some(reload_source),
        reload_args: args.clone(),
    })
}

fn build_remote_settings(config: &EspejismoConfig, args: &Args) -> Result<RemoteSettings> {
    let stealth_frame_size = config.shared.stealth.frame_size;
    let stealth_tick_ms = config.shared.stealth.tick_ms;
    let obfuscation_profile = config.shared.obfuscation.profile;
    let stealth_handshake = obfuscation_profile
        .is_stealth()
        .then_some(stealth_frame_size);
    let limits = build_user_limits(&config);

    Ok(RemoteSettings {
        users: Arc::new(build_handshake_users(&config, args, stealth_handshake)?),
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
        cold_start_delay: Duration::from_millis(
            args.cold_start_delay_ms
                .unwrap_or(config.remote.cold_start_delay_ms),
        ),
        fallback_http: FallbackHttpRuntime {
            enabled: matches!(
                config.remote.fallback_http.mode,
                ProbeDefenseMode::HttpFallback
            ) || config.remote.fallback_http.enabled,
            upstream: config.remote.fallback_http.upstream.clone(),
            probe_timeout: Duration::from_millis(config.remote.fallback_http.probe_timeout_ms),
            server: config.remote.fallback_http.server.clone(),
            body: config.remote.fallback_http.body.clone(),
        },
        egress: config.remote.egress.clone().into(),
        idle_timeout: Duration::from_secs(config.shared.idle_timeout_secs),
        max_streams: config.shared.max_streams,
        limits,
    })
}

fn build_user_limits(config: &EspejismoConfig) -> UserLimitRegistry {
    UserLimitRegistry::new(config.remote.users.iter().map(|user| {
        (
            user.name.clone(),
            UserLimitConfig {
                quota_bytes: user.quota.bytes,
                quota_window: Duration::from_secs(user.quota.window_secs),
                bandwidth_bytes_per_sec: user.bandwidth.bytes_per_sec,
            },
        )
    }))
}

impl RemoteRuntime {
    fn reload_action(&self) -> Option<AdminAction> {
        let source = self.reload_source.clone();
        let settings = self.settings.clone();
        let args = self.reload_args.clone();
        let action: AdminAction = Arc::new(move |body: Option<String>| {
            let source = source.clone();
            let settings = settings.clone();
            let args = args.clone();
            Box::pin(async move {
                let mut config = if let Some(body) = body {
                    parse_config(&body)?
                } else {
                    load_config(
                        source
                            .context("reload requires --config or --config-base64; use /apply")?,
                    )?
                };
                apply_log_overrides(&mut config.logging, &args)?;
                let next = build_remote_settings(&config, &args)?;
                let user_count = next.users.len();
                *settings.write().await = next;
                Ok(json!({
                    "ok": true,
                    "applied": true,
                    "users": user_count,
                    "applies_to": "new physical tunnels and newly opened logical streams",
                    "restart_required_for": ["listen", "admin.listen", "logging.file"]
                }))
            })
        });
        Some(action)
    }
}

async fn handle_peer(
    mut inbound: TcpStream,
    runtime: RemoteRuntime,
    replay: Arc<tokio::sync::Mutex<ReplayCache>>,
    tarpit: tarpit::TarpitManager,
    metrics: Metrics,
) -> Result<()> {
    let settings = runtime.settings.read().await.clone();
    if should_route_to_http_fallback(&mut inbound, &settings.fallback_http).await? {
        route_http_fallback(inbound, &settings.fallback_http).await?;
        return Ok(());
    }

    metrics.inc_active_physical();
    let keys = match timeout(
        settings.handshake_timeout,
        accept_handshake_with_users(&mut inbound, &settings.users, replay),
    )
    .await
    {
        Ok(Ok(keys)) => {
            metrics.inc_handshake_success();
            metrics.inc_user_handshake_success(&keys.user);
            keys
        }
        Ok(Err(err)) => {
            metrics.inc_handshake_failure();
            metrics.dec_active_physical();
            fallback_or_reject(
                inbound,
                &settings.fallback_http,
                settings.reject_delay,
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
                &settings.fallback_http,
                settings.reject_delay,
                &tarpit,
            )
            .await;
            return Err(err.into());
        }
    };

    if !settings.cold_start_delay.is_zero() {
        sleep(settings.cold_start_delay).await;
    }

    let user = keys.user;
    info!(user = %user, "authenticated tunnel accepted");
    let transport = spawn_frame_transport(
        inbound,
        keys.keys,
        settings.frames.clone(),
        runtime.tunnel_buffer,
    );
    let mut session = Session::new_server(transport, YamuxConfig::default());
    let stream_limit = Arc::new(Semaphore::new(settings.max_streams as usize));
    while let Some(stream) = session.next().await {
        let stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                metrics.dec_active_physical();
                return Err(err.into());
            }
        };

        if settings.max_streams == 0 {
            debug!(
                max = settings.max_streams,
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
        let current = runtime.settings.read().await.clone();
        let egress = current.egress.clone();
        let limits = current.limits.clone();
        let idle = current.idle_timeout;
        let user = user.clone();
        tokio::spawn(async move {
            let _stream_permit = stream_permit;
            if let Err(err) = handle_mux_stream(stream, metrics, egress, limits, idle, user).await {
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
    limits: UserLimitRegistry,
    idle: Duration,
    user: String,
) -> Result<()> {
    limits.ensure_open(&user).await?;
    metrics.inc_active_stream();
    metrics.inc_stream_opened();
    metrics.inc_user_stream_opened(&user);
    let result =
        handle_mux_stream_inner(&mut stream, metrics.clone(), egress, limits, idle, &user).await;
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
    limits: UserLimitRegistry,
    idle: Duration,
    user: &str,
) -> Result<()> {
    match read_tunnel_request(stream).await? {
        TunnelRequest::TcpConnect { authority } => {
            let mut remote = connect_egress_tcp(&authority, &egress).await?;
            info!(target = %authority, "mux TCP relay opened");
            let (client_to_remote, remote_to_client) =
                limited_copy_bidirectional(stream, &mut remote, idle, &limits, user).await?;
            metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
            metrics.add_user_tunnel_bytes(user, client_to_remote, remote_to_client);
        }
        TunnelRequest::UdpDatagram { authority, payload } => {
            limits
                .account_and_throttle(user, payload.len() as u64)
                .await?;
            let response = relay_udp_datagram(&authority, &payload, &egress, idle).await?;
            limits
                .account_and_throttle(user, response.len() as u64)
                .await?;
            metrics.add_tunnel_bytes(payload.len() as u64, response.len() as u64);
            metrics.add_user_tunnel_bytes(user, payload.len() as u64, response.len() as u64);
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

async fn limited_copy_bidirectional<A, B>(
    a: &mut A,
    b: &mut B,
    idle: Duration,
    limits: &UserLimitRegistry,
    user: &str,
) -> Result<(u64, u64)>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let mut buf_a = [0_u8; 8192];
    let mut buf_b = [0_u8; 8192];
    let mut total_a = 0_u64;
    let mut total_b = 0_u64;
    let mut a_done = false;
    let mut b_done = false;

    loop {
        let read_a = if !a_done {
            Some(timeout(idle, a.read(&mut buf_a)))
        } else {
            None
        };
        let read_b = if !b_done {
            Some(timeout(idle, b.read(&mut buf_b)))
        } else {
            None
        };

        match (read_a, read_b) {
            (Some(ra), Some(rb)) => {
                tokio::select! {
                    r = ra => {
                        match r {
                            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => {
                                a_done = true;
                                let _ = b.shutdown().await;
                                if b_done {
                                    break;
                                }
                            }
                            Ok(Ok(n)) => {
                                limits.account_and_throttle(user, n as u64).await?;
                                b.write_all(&buf_a[..n]).await?;
                                total_a += n as u64;
                            }
                        }
                        continue;
                    }
                    r = rb => {
                        match r {
                            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => {
                                b_done = true;
                                let _ = a.shutdown().await;
                                if a_done {
                                    break;
                                }
                            }
                            Ok(Ok(n)) => {
                                limits.account_and_throttle(user, n as u64).await?;
                                a.write_all(&buf_b[..n]).await?;
                                total_b += n as u64;
                            }
                        }
                        continue;
                    }
                }
            }
            (Some(ra), None) => match ra.await {
                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                Ok(Ok(n)) => {
                    limits.account_and_throttle(user, n as u64).await?;
                    b.write_all(&buf_a[..n]).await?;
                    total_a += n as u64;
                }
            },
            (None, Some(rb)) => match rb.await {
                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                Ok(Ok(n)) => {
                    limits.account_and_throttle(user, n as u64).await?;
                    a.write_all(&buf_b[..n]).await?;
                    total_b += n as u64;
                }
            },
            (None, None) => break,
        }
    }

    Ok((total_a, total_b))
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
    if let Some(proxy) = &egress.socks5_proxy {
        return relay_udp_via_socks5_proxy(proxy, authority, payload, idle).await;
    }
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

async fn relay_udp_via_socks5_proxy(
    proxy: &str,
    authority: &str,
    payload: &[u8],
    idle: Duration,
) -> Result<Vec<u8>> {
    let (host, port) = espejismo_core::split_authority(authority)?;
    let mut control = TcpStream::connect(proxy)
        .await
        .with_context(|| format!("connect SOCKS5 proxy {proxy}"))?;
    control.write_all(&[0x05, 0x01, 0x00]).await?;
    let mut method = [0_u8; 2];
    control.read_exact(&mut method).await?;
    anyhow::ensure!(
        method == [0x05, 0x00],
        "SOCKS5 proxy rejected no-auth method"
    );

    control
        .write_all(&[0x05, 0x03, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;
    let relay = read_socks5_reply_addr(&mut control).await?;
    let bind = if relay.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind).await?;
    socket.connect(relay).await?;

    let request = encode_socks5_udp_datagram(&host, port, payload)?;
    socket.send(&request).await?;
    let mut response = vec![0_u8; 65_535];
    let n = timeout(
        idle.min(Duration::from_secs(10)),
        socket.recv(&mut response),
    )
    .await??;
    decode_socks5_udp_datagram(&response[..n])
}

async fn read_socks5_reply_addr(stream: &mut TcpStream) -> Result<SocketAddr> {
    let mut head = [0_u8; 4];
    stream.read_exact(&mut head).await?;
    anyhow::ensure!(
        head[0] == 0x05 && head[1] == 0x00,
        "SOCKS5 UDP ASSOCIATE failed"
    );
    let mut host = match head[3] {
        0x01 => {
            let mut ip = [0_u8; 4];
            stream.read_exact(&mut ip).await?;
            std::net::IpAddr::from(ip)
        }
        0x04 => {
            let mut ip = [0_u8; 16];
            stream.read_exact(&mut ip).await?;
            std::net::IpAddr::from(ip)
        }
        atyp => anyhow::bail!("SOCKS5 proxy returned unsupported UDP relay address type {atyp}"),
    };
    let port = stream.read_u16().await?;
    if host.is_unspecified() {
        host = stream.peer_addr()?.ip();
    }
    Ok(SocketAddr::new(host, port))
}

fn encode_socks5_udp_datagram(host: &str, port: u16, payload: &[u8]) -> Result<Vec<u8>> {
    let host_bytes = host.as_bytes();
    anyhow::ensure!(
        host_bytes.len() <= u8::MAX as usize,
        "SOCKS5 UDP target host too long"
    );
    let mut out = Vec::with_capacity(6 + host_bytes.len() + payload.len());
    out.extend_from_slice(&[0x00, 0x00, 0x00, 0x03, host_bytes.len() as u8]);
    out.extend_from_slice(host_bytes);
    out.extend_from_slice(&port.to_be_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

fn decode_socks5_udp_datagram(input: &[u8]) -> Result<Vec<u8>> {
    anyhow::ensure!(input.len() >= 6, "SOCKS5 UDP response too short");
    anyhow::ensure!(
        input[0] == 0 && input[1] == 0 && input[2] == 0,
        "SOCKS5 UDP response has unsupported fragmentation"
    );
    let mut offset = 4;
    match input[3] {
        0x01 => offset += 4,
        0x03 => {
            anyhow::ensure!(input.len() > offset, "SOCKS5 UDP domain length missing");
            offset += 1 + input[offset] as usize;
        }
        0x04 => offset += 16,
        atyp => anyhow::bail!("SOCKS5 UDP response has unsupported address type {atyp}"),
    }
    offset += 2;
    anyhow::ensure!(input.len() >= offset, "SOCKS5 UDP response truncated");
    Ok(input[offset..].to_vec())
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
    use super::{
        build_builtin_fallback_response, decode_socks5_udp_datagram, encode_socks5_udp_datagram,
        looks_like_http_probe, FallbackHttpRuntime,
    };
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

    #[test]
    fn socks5_udp_datagram_codec_roundtrips_payload() {
        let encoded = encode_socks5_udp_datagram("example.com", 443, b"payload").unwrap();
        assert_eq!(&encoded[..5], &[0, 0, 0, 3, 11]);
        assert_eq!(decode_socks5_udp_datagram(&encoded).unwrap(), b"payload");
    }
}
