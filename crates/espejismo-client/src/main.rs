use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use espejismo_core::config::example_config;
use espejismo_core::{
    apply_log_overrides, apply_named_profile, bind_tcp_listener, config_to_toml,
    decode_profile_url, encode_config_base64, encode_profile_url, init_logging, load_config,
    load_config_base64, parse_psk, print_update_check, report_config_check, spawn_admin_server,
    AdminAction, AdminState, ClientProfile, ConfigInput, EspejismoConfig, FrameOptionOverrides,
    FrameOptions, HandshakeConfig, LogOverrides, Metrics, ProxyAuth, RuntimeState, TcpConfig,
    TunnelPoolConfig,
};
use serde_json::json;
use tokio::net::lookup_host;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tracing::{debug, info};

mod handler;
mod mux;
mod route;
mod tun;
mod tunnel;

use handler::{handle_http_client, handle_socks5_client};
use tunnel::{TunnelManager, TunnelManagerConfig, TunnelService};

#[derive(Parser, Clone, Debug)]
#[command(name = "espejismo-local", version)]
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
    profile: Option<String>,
    #[arg(long)]
    print_config_base64: bool,
    #[arg(long)]
    print_config: bool,
    #[arg(long)]
    write_config: Option<PathBuf>,
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
    #[arg(long)]
    tun_route_cleanup: bool,
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
    tunnel_min_connections: Option<usize>,
    #[arg(long)]
    tunnel_max_connections: Option<usize>,
    #[arg(long)]
    tunnel_interactive_lanes: Option<usize>,
    #[arg(long)]
    tunnel_bulk_lanes: Option<usize>,
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
    mux: espejismo_core::mux::MuxRuntimeConfig,
    tunnel_buffer: usize,
    tunnel_pool: TunnelPoolConfig,
    auth: Option<ProxyAuth>,
    tun: espejismo_core::config::LocalTunConfig,
    admin_listen: Option<SocketAddr>,
    admin_token: Option<String>,
    idle_timeout: Duration,
}

struct LocalRuntimeService {
    runtime: RwLock<LocalRuntime>,
    tunnel: Arc<TunnelService>,
}

impl LocalRuntimeService {
    fn new(runtime: LocalRuntime, metrics: Metrics, runtime_state: RuntimeState) -> Self {
        let manager = build_tunnel_manager(&runtime, metrics, runtime_state);
        Self {
            runtime: RwLock::new(runtime),
            tunnel: Arc::new(TunnelService::new(manager)),
        }
    }

    async fn snapshot(&self) -> LocalRuntime {
        self.runtime.read().await.clone()
    }

    async fn apply(&self, runtime: LocalRuntime, metrics: Metrics, runtime_state: RuntimeState) {
        let manager = build_tunnel_manager(&runtime, metrics, runtime_state);
        self.tunnel.replace(manager).await;
        *self.runtime.write().await = runtime;
    }
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
        let mut example_config = espejismo_core::parse_config(&example_config())?;
        if let Some(profile) = &args.profile {
            apply_named_profile(&mut example_config, profile)?;
        }
        let example = config_to_toml(&example_config)?;
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
    if let Some(profile_url) = &args.import_profile {
        decode_profile_url(profile_url)?.apply_to_config(&mut config);
    }
    if let Some(profile) = &args.profile {
        apply_named_profile(&mut config, profile)?;
    }
    apply_cli_overrides_to_config(&mut config, &args)?;
    if args.tun_route_cleanup {
        route::cleanup_tun_routes(&config.local.tun).await?;
        println!("TUN route cleanup completed for {}", config.local.tun.name);
        return Ok(());
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
    if args.print_config {
        print!("{}", config_to_toml(&config)?);
        return Ok(());
    }
    if let Some(path) = &args.write_config {
        std::fs::write(path, config_to_toml(&config)?)
            .with_context(|| format!("write {}", path.display()))?;
        println!("wrote {}", path.display());
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
    let service = Arc::new(LocalRuntimeService::new(
        runtime.clone(),
        metrics.clone(),
        runtime_state.clone(),
    ));
    if let Some(addr) = runtime.admin_listen {
        let reload = local_reload_action(
            config_input,
            args.clone(),
            service.clone(),
            metrics.clone(),
            runtime_state.clone(),
        );
        spawn_admin_server(
            addr,
            AdminState {
                role: "local".to_string(),
                metrics: metrics.clone(),
                runtime: runtime_state.clone(),
                token: runtime.admin_token.clone(),
                reload: Some(reload),
            },
        );
    }

    let mut listeners = JoinSet::new();
    if let Some(addr) = runtime.socks5_listen {
        let listener = bind_tcp_listener(addr, &runtime.tcp)?;
        let service = service.clone();
        let metrics = metrics.clone();
        listeners.spawn(async move {
            info!(listen = %addr, "SOCKS5 proxy listening");
            loop {
                let (socket, peer) = listener.accept().await?;
                let current = service.snapshot().await;
                let _ = espejismo_core::apply_tcp_options(&socket, &current.tcp);
                let tunnel = service.tunnel.clone();
                let auth = current.auth.clone();
                let idle = current.idle_timeout;
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
        let service = service.clone();
        let metrics = metrics.clone();
        listeners.spawn(async move {
            info!(listen = %addr, "HTTP proxy listening");
            loop {
                let (socket, peer) = listener.accept().await?;
                let current = service.snapshot().await;
                let _ = espejismo_core::apply_tcp_options(&socket, &current.tcp);
                let tunnel = service.tunnel.clone();
                let auth = current.auth.clone();
                let idle = current.idle_timeout;
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
            service.tunnel.clone(),
            metrics.clone(),
            runtime.idle_timeout,
        ));
    }

    anyhow::ensure!(
        !listeners.is_empty(),
        "enable at least one local ingress: socks5_listen, http_listen, or local.tun.enabled"
    );
    info!(server = %runtime.server, mux = ?runtime.mux.mode, "local proxy ready with reconnecting tunnel manager");

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

fn build_tunnel_manager(
    runtime: &LocalRuntime,
    metrics: Metrics,
    runtime_state: RuntimeState,
) -> TunnelManager {
    TunnelManager::new(
        TunnelManagerConfig {
            server: runtime.server.clone(),
            handshake: runtime.handshake.clone(),
            frames: runtime.frames.clone(),
            tcp: runtime.tcp.clone(),
            mux: runtime.mux,
            tunnel_buffer: runtime.tunnel_buffer,
            pool: runtime.tunnel_pool.clone(),
        },
        metrics,
        runtime_state,
    )
}

fn local_reload_action(
    source: ConfigInput,
    args: Args,
    service: Arc<LocalRuntimeService>,
    metrics: Metrics,
    runtime_state: RuntimeState,
) -> AdminAction {
    Arc::new(move |body| {
        let source = source.clone();
        let args = args.clone();
        let service = service.clone();
        let metrics = metrics.clone();
        let runtime_state = runtime_state.clone();
        Box::pin(async move {
            let mut config = match body {
                Some(toml) => {
                    toml::from_str::<EspejismoConfig>(&toml).context("parse local apply config")?
                }
                None => load_config(source)
                    .context("reload requires --config or --config-base64; use /apply")?,
            };
            if let Some(profile_url) = &args.import_profile {
                decode_profile_url(profile_url)?.apply_to_config(&mut config);
            }
            apply_log_overrides(&mut config.logging, &log_overrides(&args))?;
            let runtime = build_runtime(config, &args)?;
            service
                .apply(runtime, metrics.clone(), runtime_state.clone())
                .await;
            runtime_state.mark_config_applied();
            Ok(json!({
                "ok": true,
                "applied": true,
                "role": "local",
                "runtime_updated": [
                    "server",
                    "local.auth",
                    "shared.tcp",
                    "shared.pacing",
                    "shared.obfuscation",
                    "shared.mux",
                    "local.tunnel_pool"
                ],
                "restart_required_for": [
                    "local.socks5_listen",
                    "local.http_listen",
                    "local.tun",
                    "admin.listen",
                    "logging.file"
                ]
            }))
        })
    })
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

fn apply_tun_cli_overrides(config: &mut EspejismoConfig, args: &Args) {
    if args.tun_enabled {
        config.local.tun.enabled = true;
    }
    if let Some(value) = &args.tun_name {
        config.local.tun.name = value.clone();
    }
    if let Some(value) = args.tun_address {
        config.local.tun.address = value;
    }
    if let Some(value) = args.tun_destination {
        config.local.tun.destination = value;
    }
    if let Some(value) = args.tun_prefix {
        config.local.tun.prefix = value;
    }
    if let Some(value) = args.tun_mtu {
        config.local.tun.mtu = value;
    }
    if args.tun_auto_route {
        config.local.tun.route.enabled = true;
    }
    if args.tun_auto_dns {
        config.local.tun.route.dns_enabled = true;
    }
    if !args.tun_dns.is_empty() {
        config.local.tun.route.dns_servers = args.tun_dns.clone();
    }
}

fn apply_cli_overrides_to_config(config: &mut EspejismoConfig, args: &Args) -> Result<()> {
    if let Some(value) = &args.psk {
        config.shared.psk = Some(value.clone());
    }
    if let Some(value) = args.clock_skew_secs {
        config.shared.clock_skew_secs = value;
    }
    if let Some(value) = args.max_padding {
        config.shared.max_padding = value;
    }
    if let Some(value) = args.jitter_ms {
        config.shared.jitter_ms = value;
    }
    if let Some(value) = args.padding_chance_percent {
        config.shared.padding_chance_percent = value;
    }
    if let Some(value) = args.backpressure_threshold_ms {
        config.shared.backpressure_threshold_ms = value;
    }
    if let Some(value) = args.backpressure_cooldown_ms {
        config.shared.backpressure_cooldown_ms = value;
    }
    if let Some(value) = args.puzzle_bits {
        config.shared.puzzle_bits = value;
    }
    if let Some(value) = args.tunnel_buffer {
        config.shared.tunnel_buffer = value;
    }
    if let Some(value) = &args.server {
        config.local.server = Some(value.clone());
    }
    if let Some(value) = args.socks5_listen {
        config.local.socks5_listen = Some(value);
    }
    if let Some(value) = args.http_listen {
        config.local.http_listen = Some(value);
    }
    if let Some(value) = args.handshake_padding {
        config.local.handshake_padding = value;
    }
    if let Some(value) = args.tunnel_min_connections {
        config.local.tunnel_pool.min_connections = value;
    }
    if let Some(value) = args.tunnel_max_connections {
        config.local.tunnel_pool.max_connections = value;
    }
    if let Some(value) = args.tunnel_interactive_lanes {
        config.local.tunnel_pool.interactive_lanes = value;
    }
    if let Some(value) = args.tunnel_bulk_lanes {
        config.local.tunnel_pool.bulk_lanes = value;
    }
    if let Some(value) = args.admin_listen {
        config.admin.listen = Some(value);
    }
    if let Some(value) = &args.admin_token {
        config.admin.token = Some(value.clone());
    }
    apply_tun_cli_overrides(config, args);
    apply_log_overrides(&mut config.logging, &log_overrides(args))?;
    let normalized = config_to_toml(config)?;
    *config = espejismo_core::parse_config(&normalized)?;
    Ok(())
}

fn build_runtime(config: EspejismoConfig, args: &Args) -> Result<LocalRuntime> {
    let psk = args
        .psk
        .clone()
        .or_else(|| config.shared.psk.clone())
        .context("provide psk in config, --psk, or ESPEJISMO_PSK")?;
    let server = args
        .server
        .clone()
        .or(config.local.server)
        .context("provide local.server in config or --server")?;
    let stealth_frame_size = config.shared.stealth.frame_size;
    let obfuscation_profile = config.shared.obfuscation.profile;
    let stealth_handshake = obfuscation_profile
        .is_stealth()
        .then_some(stealth_frame_size);

    let mut tun = config.local.tun;
    let mut tunnel_pool = config.local.tunnel_pool;
    if let Some(value) = args.tunnel_min_connections {
        tunnel_pool.min_connections = value;
    }
    if let Some(value) = args.tunnel_max_connections {
        tunnel_pool.max_connections = value;
    }
    if let Some(value) = args.tunnel_interactive_lanes {
        tunnel_pool.interactive_lanes = value;
    }
    if let Some(value) = args.tunnel_bulk_lanes {
        tunnel_pool.bulk_lanes = value;
    }
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

    let mux = espejismo_core::mux::MuxRuntimeConfig::from_config(
        config.shared.max_streams,
        &config.shared.mux,
    );

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
        .with_stealth_frame_size(stealth_handshake)
        .with_mux_mode(config.shared.mux.mode),
        frames: config.shared.frame_options(&FrameOptionOverrides {
            max_padding: args.max_padding,
            jitter_ms: args.jitter_ms,
            padding_chance_percent: args.padding_chance_percent,
            backpressure_threshold_ms: args.backpressure_threshold_ms,
            backpressure_cooldown_ms: args.backpressure_cooldown_ms,
        }),
        tcp: config.shared.tcp.clone(),
        mux,
        tunnel_buffer: args.tunnel_buffer.unwrap_or(config.shared.tunnel_buffer),
        tunnel_pool,
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
    let mut pool = config.local.tunnel_pool.clone();
    if let Some(value) = args.tunnel_min_connections {
        pool.min_connections = value;
    }
    if let Some(value) = args.tunnel_max_connections {
        pool.max_connections = value;
    }
    if let Some(value) = args.tunnel_interactive_lanes {
        pool.interactive_lanes = value;
    }
    if let Some(value) = args.tunnel_bulk_lanes {
        pool.bulk_lanes = value;
    }
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
    if pool.max_connections == 0 {
        errors.push("local.tunnel_pool.max_connections must be greater than 0".to_string());
    }
    if pool.min_connections > pool.max_connections {
        errors.push("local.tunnel_pool.min_connections must be <= max_connections".to_string());
    }
    if pool.interactive_lanes + pool.bulk_lanes == 0 {
        errors.push("local.tunnel_pool must configure at least one lane".to_string());
    }
    if pool.interactive_lanes + pool.bulk_lanes > pool.max_connections {
        errors.push(
            "local.tunnel_pool interactive_lanes + bulk_lanes must be <= max_connections"
                .to_string(),
        );
    }
    if errors.is_empty() {
        println!(
            "OK tunnel pool lanes: min={}, max={}, interactive={}, bulk={}",
            pool.min_connections, pool.max_connections, pool.interactive_lanes, pool.bulk_lanes
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
            "TUN mode requires OS privileges; --tun-auto-route changes system routes".to_string(),
        );
        println!("OK TUN ingress requested");
    }
    if tun_route_enabled {
        #[cfg(target_os = "linux")]
        println!("OK Linux TUN auto-route requested");
        #[cfg(target_os = "macos")]
        println!("OK macOS TUN auto-route requested");
        #[cfg(target_os = "windows")]
        println!("OK Windows TUN auto-route requested");
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        errors.push(
            "TUN auto-route is currently implemented only on Linux, macOS, and Windows".to_string(),
        );
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
        #[cfg(target_os = "macos")]
        println!("OK macOS TUN auto-DNS requested");
        #[cfg(target_os = "windows")]
        println!("OK Windows TUN auto-DNS requested");
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        errors.push(
            "TUN auto-DNS is currently implemented only on Linux, macOS, and Windows".to_string(),
        );
    }
    report_config_check(warnings, errors)
}
