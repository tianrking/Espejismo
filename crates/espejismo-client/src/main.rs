use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use espejismo_core::config::example_config;
use espejismo_core::{
    apply_log_overrides, bind_tcp_listener, config_to_toml, connect_handshake, connect_tcp_stream,
    decode_profile_url, encode_config_base64, encode_profile_url, http_proxy,
    idle_copy_bidirectional, init_logging, load_config, load_config_base64, parse_psk,
    print_update_check, report_config_check, socks5, spawn_admin_server, spawn_frame_transport,
    write_tcp_connect, write_udp_datagram, AdminState, ClientProfile, ConfigInput, EspejismoConfig,
    FrameOptions, HandshakeConfig, LogOverrides, Metrics, ProxyAuth, RuntimeState, TcpConfig,
};
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{lookup_host, TcpStream, UdpSocket};
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_yamux::{Config as YamuxConfig, Control, Session, StreamHandle};
use tracing::{debug, info};

mod route;
mod tun;

#[derive(Parser, Debug)]
#[command(name = "espejismo-local")]
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
    print_config_base64: bool,
    #[arg(long)]
    decode_config_base64: Option<String>,
    #[arg(long)]
    check_config: bool,
    #[arg(long)]
    check_update: bool,
    #[arg(long)]
    update_url: Option<String>,
    #[arg(long)]
    print_client_profile: bool,
    #[arg(long, default_value = "default")]
    profile_name: String,
    #[arg(long)]
    import_profile: Option<String>,
    #[arg(long)]
    socks5_listen: Option<SocketAddr>,
    #[arg(long)]
    http_listen: Option<SocketAddr>,
    #[arg(long)]
    tun_enabled: bool,
    #[arg(long)]
    tun_name: Option<String>,
    #[arg(long)]
    tun_address: Option<std::net::Ipv4Addr>,
    #[arg(long)]
    tun_destination: Option<std::net::Ipv4Addr>,
    #[arg(long)]
    tun_prefix: Option<u8>,
    #[arg(long)]
    tun_mtu: Option<u16>,
    #[arg(long)]
    tun_auto_route: bool,
    #[arg(long)]
    tun_auto_dns: bool,
    #[arg(long, value_delimiter = ',')]
    tun_dns: Vec<IpAddr>,
    #[arg(long)]
    server: Option<String>,
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
    handshake_padding: Option<usize>,
    #[arg(long)]
    puzzle_bits: Option<u8>,
    #[arg(long)]
    tunnel_buffer: Option<usize>,
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
struct LocalRuntime {
    server: String,
    socks5_listen: Option<SocketAddr>,
    http_listen: Option<SocketAddr>,
    handshake: HandshakeConfig,
    frames: FrameOptions,
    tcp: TcpConfig,
    tunnel_buffer: usize,
    auth: Option<ProxyAuth>,
    tun: espejismo_core::config::LocalTunConfig,
    admin_listen: Option<SocketAddr>,
    admin_token: Option<String>,
    idle_timeout: Duration,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.check_update {
        print_update_check(args.update_url.as_deref())?;
        return Ok(());
    }
    if let Some(encoded) = &args.decode_config_base64 {
        let config = load_config_base64(encoded)?;
        print!("{}", config_to_toml(&config)?);
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

    let mut config = load_config(ConfigInput {
        path: args.config.clone(),
        base64: args.config_base64.clone(),
    })?;
    if let Some(profile_url) = &args.import_profile {
        decode_profile_url(profile_url)?.apply_to_config(&mut config);
    }
    if args.check_config {
        check_local_config(&config, &args).await?;
        return Ok(());
    }
    if args.print_client_profile {
        let profile = ClientProfile::from_config(args.profile_name.clone(), &config)?;
        println!("{}", encode_profile_url(&profile)?);
        return Ok(());
    }
    if args.print_config_base64 {
        println!("{}", encode_config_base64(&config_to_toml(&config)?));
        return Ok(());
    }
    apply_log_overrides(&mut config.logging, &log_overrides(&args))?;
    let _log_guard = init_logging(&config.logging)?;
    let runtime = build_runtime(config, &args)?;
    let metrics = Metrics::default();
    let runtime_state = RuntimeState::default();
    if let Some(addr) = runtime.admin_listen {
        spawn_admin_server(
            addr,
            AdminState {
                role: "local".to_string(),
                metrics: metrics.clone(),
                runtime: runtime_state.clone(),
                token: runtime.admin_token.clone(),
                reload: None,
            },
        );
    }

    let tunnel = Arc::new(TunnelManager::new(
        runtime.server.clone(),
        runtime.handshake,
        runtime.frames,
        runtime.tcp.clone(),
        runtime.tunnel_buffer,
        metrics.clone(),
        runtime_state.clone(),
    ));

    let mut listeners = JoinSet::new();
    if let Some(addr) = runtime.socks5_listen {
        let listener = bind_tcp_listener(addr, &runtime.tcp)?;
        let tunnel = tunnel.clone();
        let auth = runtime.auth.clone();
        let metrics = metrics.clone();
        let idle = runtime.idle_timeout;
        let tcp = runtime.tcp.clone();
        listeners.spawn(async move {
            info!(listen = %addr, "SOCKS5 proxy listening");
            loop {
                let (socket, peer) = listener.accept().await?;
                let _ = espejismo_core::apply_tcp_options(&socket, &tcp);
                let tunnel = tunnel.clone();
                let auth = auth.clone();
                let metrics = metrics.clone();
                metrics.inc_accepted();
                tokio::spawn(async move {
                    if let Err(err) =
                        handle_socks5_client(socket, tunnel, auth, metrics, idle).await
                    {
                        debug!(%peer, error = %err, "SOCKS5 connection ended");
                    }
                });
            }
            #[allow(unreachable_code)]
            Ok::<_, anyhow::Error>(())
        });
    }

    if let Some(addr) = runtime.http_listen {
        let listener = bind_tcp_listener(addr, &runtime.tcp)?;
        let tunnel = tunnel.clone();
        let auth = runtime.auth.clone();
        let metrics = metrics.clone();
        let idle = runtime.idle_timeout;
        let tcp = runtime.tcp.clone();
        listeners.spawn(async move {
            info!(listen = %addr, "HTTP proxy listening");
            loop {
                let (socket, peer) = listener.accept().await?;
                let _ = espejismo_core::apply_tcp_options(&socket, &tcp);
                let tunnel = tunnel.clone();
                let auth = auth.clone();
                let metrics = metrics.clone();
                metrics.inc_accepted();
                tokio::spawn(async move {
                    if let Err(err) = handle_http_client(socket, tunnel, auth, metrics, idle).await
                    {
                        debug!(%peer, error = %err, "HTTP proxy connection ended");
                    }
                });
            }
            #[allow(unreachable_code)]
            Ok::<_, anyhow::Error>(())
        });
    }

    if runtime.tun.enabled {
        listeners.spawn(tun::run_tun_ingress(
            runtime.tun.clone(),
            runtime.server.clone(),
            tunnel.clone(),
            metrics.clone(),
            runtime.idle_timeout,
        ));
    }

    anyhow::ensure!(
        !listeners.is_empty(),
        "enable at least one local ingress: socks5_listen, http_listen, or local.tun.enabled"
    );
    info!(server = %runtime.server, "local proxy ready with reconnecting yamux tunnel manager");

    tokio::select! {
        result = listeners.join_next() => {
            if let Some(result) = result {
                result??;
            }
        }
        _ = shutdown_signal() => {
            info!("shutdown signal received");
            listeners.abort_all();
            while listeners.join_next().await.is_some() {}
        }
    }
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let terminate = async {
            if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
                sigterm.recv().await;
            }
        };
        tokio::select! {
            _ = ctrl_c => {}
            _ = terminate => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }
}

fn log_overrides(args: &Args) -> LogOverrides {
    LogOverrides {
        level: args.log_level.clone(),
        format: args.log_format.clone(),
        file: args.log_file.clone(),
        no_ansi: args.no_log_ansi,
    }
}

fn build_runtime(config: EspejismoConfig, args: &Args) -> Result<LocalRuntime> {
    let psk = args
        .psk
        .clone()
        .or(config.shared.psk)
        .context("provide psk in config, --psk, or ESPEJISMO_PSK")?;
    let server = args
        .server
        .clone()
        .or(config.local.server)
        .context("provide local.server in config or --server")?;
    let stealth_frame_size = config.shared.stealth.frame_size;
    let stealth_tick_ms = config.shared.stealth.tick_ms;
    let obfuscation_profile = config.shared.obfuscation.profile;
    let stealth_handshake = obfuscation_profile
        .is_stealth()
        .then_some(stealth_frame_size);

    let mut tun = config.local.tun;
    if args.tun_enabled {
        tun.enabled = true;
    }
    if let Some(name) = &args.tun_name {
        tun.name = name.clone();
    }
    if let Some(address) = args.tun_address {
        tun.address = address;
    }
    if let Some(destination) = args.tun_destination {
        tun.destination = destination;
    }
    if let Some(prefix) = args.tun_prefix {
        tun.prefix = prefix;
    }
    if let Some(mtu) = args.tun_mtu {
        tun.mtu = mtu;
    }
    if args.tun_auto_route {
        tun.route.enabled = true;
    }
    if args.tun_auto_dns {
        tun.route.dns_enabled = true;
    }
    if !args.tun_dns.is_empty() {
        tun.route.dns_servers = args.tun_dns.clone();
    }

    Ok(LocalRuntime {
        server,
        socks5_listen: args.socks5_listen.or(config.local.socks5_listen),
        http_listen: args.http_listen.or(config.local.http_listen),
        handshake: HandshakeConfig::new(
            parse_psk(&psk)?,
            args.clock_skew_secs
                .unwrap_or(config.shared.clock_skew_secs),
            args.handshake_padding
                .unwrap_or(config.local.handshake_padding),
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
            pacing_enabled: config.shared.pacing.enabled,
            pacing_max_bytes_per_sec: config.shared.pacing.max_bytes_per_sec,
            pacing_burst_bytes: config.shared.pacing.burst_bytes,
            pacing_min_write_bytes: config.shared.pacing.min_write_bytes,
            heartbeat_secs: config.shared.tcp.heartbeat_secs,
        },
        tcp: config.shared.tcp.clone(),
        tunnel_buffer: args.tunnel_buffer.unwrap_or(config.shared.tunnel_buffer),
        auth: config.local.auth,
        tun,
        admin_listen: args.admin_listen.or(config.admin.listen),
        admin_token: args.admin_token.clone().or(config.admin.token),
        idle_timeout: Duration::from_secs(config.shared.idle_timeout_secs),
    })
}

async fn check_local_config(config: &EspejismoConfig, args: &Args) -> Result<()> {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let socks_addr = args.socks5_listen.or(config.local.socks5_listen);
    let http_addr = args.http_listen.or(config.local.http_listen);
    let admin_addr = args.admin_listen.or(config.admin.listen);
    let tun_enabled = args.tun_enabled || config.local.tun.enabled;
    let tun_prefix = args.tun_prefix.unwrap_or(config.local.tun.prefix);
    let tun_mtu = args.tun_mtu.unwrap_or(config.local.tun.mtu);
    let tun_route_enabled = args.tun_auto_route || config.local.tun.route.enabled;
    let tun_dns_enabled = args.tun_auto_dns || config.local.tun.route.dns_enabled;
    if socks_addr.is_some() && socks_addr == http_addr {
        errors.push(
            "local.socks5_listen and local.http_listen must use different addresses".to_string(),
        );
    }
    if admin_addr.is_some() && (admin_addr == socks_addr || admin_addr == http_addr) {
        errors.push("admin.listen must not reuse a proxy listener address".to_string());
    }
    let server = args.server.clone().or_else(|| config.local.server.clone());
    match server {
        Some(server) => match lookup_host(server.as_str()).await {
            Ok(addrs) => {
                if addrs.count() > 0 {
                    println!("OK local.server resolves: {server}");
                } else {
                    errors.push(format!("local.server resolved no addresses: {server}"));
                }
            }
            Err(err) => errors.push(format!("local.server cannot resolve {server}: {err}")),
        },
        None => errors.push("local.server is required".to_string()),
    }
    if let Some(psk) = args.psk.as_ref().or(config.shared.psk.as_ref()) {
        if psk.len() < 16 {
            warnings
                .push("PSK is shorter than 16 characters; use a longer random secret".to_string());
        } else {
            println!("OK PSK length looks usable");
        }
    } else {
        errors.push("shared.psk, --psk, or ESPEJISMO_PSK is required".to_string());
    }
    for (name, addr) in [
        ("local.socks5_listen", socks_addr),
        ("local.http_listen", http_addr),
        ("admin.listen", admin_addr),
    ] {
        if let Some(addr) = addr {
            match bind_tcp_listener(addr, &config.shared.tcp) {
                Ok(listener) => {
                    drop(listener);
                    println!("OK {name} can bind: {addr}");
                }
                Err(err) => errors.push(format!("{name} cannot bind {addr}: {err}")),
            }
        }
    }
    if config.admin.listen.is_some()
        && args
            .admin_token
            .as_ref()
            .or(config.admin.token.as_ref())
            .is_none()
    {
        warnings.push("admin.listen is enabled without an admin token".to_string());
    }
    if config.shared.pacing.enabled && config.shared.pacing.max_bytes_per_sec > 0 {
        println!(
            "OK pacing cap configured: {} bytes/sec",
            config.shared.pacing.max_bytes_per_sec
        );
    }
    if tun_enabled {
        if tun_prefix > 32 {
            errors.push("local.tun.prefix must be <= 32".to_string());
        }
        if tun_mtu < 576 {
            errors.push("local.tun.mtu must be >= 576".to_string());
        }
        warnings.push(
            "TUN mode requires OS privileges; --tun-auto-route changes the Linux default route"
                .to_string(),
        );
        println!("OK TUN ingress requested");
    }
    if tun_route_enabled {
        #[cfg(target_os = "linux")]
        println!("OK Linux TUN auto-route requested");
        #[cfg(not(target_os = "linux"))]
        errors.push("TUN auto-route is currently implemented only on Linux".to_string());
    }
    if tun_dns_enabled {
        let dns_servers = if args.tun_dns.is_empty() {
            &config.local.tun.route.dns_servers
        } else {
            &args.tun_dns
        };
        if dns_servers.is_empty() {
            errors.push(
                "local.tun.route.dns_servers must not be empty when DNS is enabled".to_string(),
            );
        }
        #[cfg(target_os = "linux")]
        println!("OK Linux TUN auto-DNS requested");
        #[cfg(not(target_os = "linux"))]
        errors.push("TUN auto-DNS is currently implemented only on Linux".to_string());
    }
    report_config_check(warnings, errors)
}

#[derive(Clone)]
struct TunnelManager {
    server: String,
    handshake: HandshakeConfig,
    frames: FrameOptions,
    tcp: TcpConfig,
    tunnel_buffer: usize,
    metrics: Metrics,
    runtime_state: RuntimeState,
    control: Arc<Mutex<Option<Control>>>,
}

impl TunnelManager {
    fn new(
        server: String,
        handshake: HandshakeConfig,
        frames: FrameOptions,
        tcp: TcpConfig,
        tunnel_buffer: usize,
        metrics: Metrics,
        runtime_state: RuntimeState,
    ) -> Self {
        Self {
            server,
            handshake,
            frames,
            tcp,
            tunnel_buffer,
            metrics,
            runtime_state,
            control: Arc::new(Mutex::new(None)),
        }
    }

    async fn open_stream(&self) -> Result<StreamHandle> {
        let mut guard = self.control.lock().await;
        if guard.is_none() {
            *guard = Some(
                connect_mux(
                    self.server.clone(),
                    self.handshake.clone(),
                    self.frames.clone(),
                    self.tcp.clone(),
                    self.tunnel_buffer,
                    self.metrics.clone(),
                    self.runtime_state.clone(),
                )
                .await?,
            );
        }
        if let Some(control) = guard.as_mut() {
            match control.open_stream().await {
                Ok(stream) => return Ok(stream),
                Err(err) => {
                    debug!(error = %err, "yamux stream open failed; reconnecting tunnel");
                    self.runtime_state
                        .record_error(format!("yamux stream open failed: {err}"));
                    *guard = None;
                }
            }
        }
        *guard = Some(
            connect_mux(
                self.server.clone(),
                self.handshake.clone(),
                self.frames.clone(),
                self.tcp.clone(),
                self.tunnel_buffer,
                self.metrics.clone(),
                self.runtime_state.clone(),
            )
            .await?,
        );
        guard
            .as_mut()
            .context("tunnel reconnect did not install control")?
            .open_stream()
            .await
            .context("open yamux stream after reconnect")
    }
}

async fn connect_mux(
    server: String,
    cfg: HandshakeConfig,
    options: FrameOptions,
    tcp: TcpConfig,
    tunnel_buffer: usize,
    metrics: Metrics,
    runtime_state: RuntimeState,
) -> Result<Control> {
    runtime_state.set_tunnel_state("connecting");
    apply_reconnect_backoff(&runtime_state).await;
    let mut upstream = match connect_tcp_stream(server.as_str(), &tcp).await {
        Ok(stream) => stream,
        Err(err) => {
            runtime_state.record_error(format!("connect {server}: {err}"));
            return Err(err);
        }
    };
    metrics.inc_active_physical();
    let keys = match connect_handshake(&mut upstream, &cfg).await {
        Ok(keys) => {
            metrics.inc_handshake_success();
            keys
        }
        Err(err) => {
            metrics.inc_handshake_failure();
            metrics.dec_active_physical();
            runtime_state.record_error(format!("handshake {server}: {err}"));
            return Err(err);
        }
    };
    runtime_state.record_connect_success();
    let transport = spawn_frame_transport(upstream, keys, options, tunnel_buffer);
    let mut session = Session::new_client(transport, YamuxConfig::default());
    let control = session.control();

    tokio::spawn(async move {
        while let Some(event) = session.next().await {
            if let Err(err) = event {
                debug!(error = %err, "yamux client session stopped");
                runtime_state.record_error(format!("yamux client session stopped: {err}"));
                break;
            }
        }
        runtime_state.set_tunnel_state("disconnected");
        metrics.dec_active_physical();
    });

    Ok(control)
}

async fn apply_reconnect_backoff(runtime_state: &RuntimeState) {
    let failures = runtime_state.snapshot().consecutive_failures;
    if failures == 0 {
        return;
    }
    let exponent = failures.min(6) as u32;
    let delay = Duration::from_millis(250_u64.saturating_mul(1_u64 << exponent));
    tokio::time::sleep(delay).await;
}

async fn handle_socks5_client(
    mut local: TcpStream,
    tunnel: Arc<TunnelManager>,
    auth: Option<ProxyAuth>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    metrics.inc_active_stream();
    metrics.inc_stream_opened();
    let result = handle_socks5_client_inner(&mut local, tunnel, auth, metrics.clone(), idle).await;
    if result.is_err() {
        metrics.inc_stream_failed();
    }
    metrics.dec_active_stream();
    result
}

async fn handle_socks5_client_inner(
    local: &mut TcpStream,
    tunnel: Arc<TunnelManager>,
    auth: Option<ProxyAuth>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    match socks5::accept_request_with_auth(local, auth.as_ref()).await? {
        socks5::SocksRequest::Connect(target) => {
            let mut stream = tunnel.open_stream().await?;
            write_tcp_connect(&mut stream, &target.authority()).await?;
            let (client_to_remote, remote_to_client) =
                idle_copy_bidirectional(local, &mut stream, idle).await?;
            metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
            Ok(())
        }
        socks5::SocksRequest::UdpAssociate => {
            handle_udp_associate(local, tunnel, metrics, idle).await
        }
    }
}

async fn handle_http_client(
    mut local: TcpStream,
    tunnel: Arc<TunnelManager>,
    auth: Option<ProxyAuth>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    metrics.inc_active_stream();
    metrics.inc_stream_opened();
    let result = handle_http_client_inner(&mut local, tunnel, auth, metrics.clone(), idle).await;
    if result.is_err() {
        metrics.inc_stream_failed();
    }
    metrics.dec_active_stream();
    result
}

async fn handle_http_client_inner(
    local: &mut TcpStream,
    tunnel: Arc<TunnelManager>,
    auth: Option<ProxyAuth>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    let target = http_proxy::accept_http_proxy_with_auth(local, auth.as_ref()).await?;
    let mut stream = tunnel.open_stream().await?;
    write_tcp_connect(&mut stream, &target.authority).await?;
    if !target.prebuffer.is_empty() {
        stream.write_all(&target.prebuffer).await?;
    }
    let (client_to_remote, remote_to_client) =
        idle_copy_bidirectional(local, &mut stream, idle).await?;
    metrics.add_tunnel_bytes(client_to_remote, remote_to_client);
    Ok(())
}

async fn handle_udp_associate(
    control_stream: &mut TcpStream,
    tunnel: Arc<TunnelManager>,
    metrics: Metrics,
    idle: Duration,
) -> Result<()> {
    let bind_addr = if control_stream.local_addr()?.is_ipv4() {
        "127.0.0.1:0"
    } else {
        "[::1]:0"
    };
    let udp = UdpSocket::bind(bind_addr).await?;
    let udp_addr = udp.local_addr()?;
    socks5::reply_udp_associate(control_stream, udp_addr).await?;
    let mut buf = vec![0_u8; 65_535];
    loop {
        let (n, peer) = timeout(idle, udp.recv_from(&mut buf)).await??;
        let packet = socks5::parse_udp_packet(&buf[..n])?;
        let response = relay_udp_packet(tunnel.clone(), &packet.target, &packet.payload).await?;
        let wrapped = socks5::build_udp_packet(&packet.target, &response)?;
        udp.send_to(&wrapped, peer).await?;
        metrics.add_tunnel_bytes(packet.payload.len() as u64, response.len() as u64);
    }
}

async fn relay_udp_packet(
    tunnel: Arc<TunnelManager>,
    target: &socks5::SocksTarget,
    payload: &[u8],
) -> Result<Vec<u8>> {
    let mut stream = tunnel.open_stream().await?;
    write_udp_datagram(&mut stream, &target.authority(), payload).await?;
    let len = timeout(Duration::from_secs(15), stream.read_u16()).await?? as usize;
    let mut response = vec![0_u8; len];
    timeout(Duration::from_secs(15), stream.read_exact(&mut response)).await??;
    Ok(response)
}
