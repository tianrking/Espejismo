use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use espejismo_core::config::example_config;
use espejismo_core::{
    apply_log_overrides, apply_named_profile, apply_tcp_options, bind_tcp_listener, config_to_toml,
    encode_config_base64, init_logging, load_config, load_config_base64, parse_config, parse_psk,
    print_update_check, report_config_check, spawn_admin_server, AdminAction, AdminState,
    ConfigInput, EgressPolicy, EspejismoConfig, FrameOptionOverrides, FrameOptions,
    HandshakeConfig, HandshakeUser, LogOverrides, Metrics, NoopTrafficObserver, ProbeDefenseMode,
    ReplayCache, RuntimeState, TcpConfig, TrafficObserver,
};
use serde_json::json;
use tokio::sync::{mpsc, RwLock, Semaphore};
use tracing::{debug, info};

const ACCEPT_RESOURCE_RETRY_BASE_DELAY: Duration = Duration::from_millis(250);
const ACCEPT_RESOURCE_RETRY_MAX_DELAY: Duration = Duration::from_secs(16);

mod fallback;
mod handler;
mod http_chain;
mod limits;
mod mux;
mod relay;
mod socks5_chain;
mod tarpit;

use fallback::FallbackHttpRuntime;
use handler::handle_peer;
use limits::{UserLimitConfig, UserLimitRegistry};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "espejismo-remote",
    version,
    about = "Remote authenticated Espejismo tunnel endpoint",
    after_help = "Examples:\n  espejismo-remote --config server.toml\n  espejismo-remote --config server.toml --check-config\n  espejismo-remote --listen 0.0.0.0:6690 --psk 'replace-with-a-long-secret'"
)]
pub(crate) struct Args {
    /// Load TOML settings from this file. Example: --config server.toml.
    #[arg(long)]
    config: Option<String>,
    /// Load TOML settings from a base64-encoded string.
    #[arg(long)]
    config_base64: Option<String>,
    /// Print a starter TOML configuration and exit.
    #[arg(long)]
    print_example_config: bool,
    /// Print a base64-encoded starter configuration and exit.
    #[arg(long)]
    print_example_config_base64: bool,
    /// Apply a built-in profile before CLI overrides, such as --profile server-safe.
    #[arg(long)]
    profile: Option<String>,
    /// Print the selected effective configuration as base64 and exit.
    #[arg(long)]
    print_config_base64: bool,
    /// Print the selected effective configuration as TOML and exit.
    #[arg(long)]
    print_config: bool,
    /// Write the selected effective configuration to a file and exit.
    #[arg(long)]
    write_config: Option<PathBuf>,
    /// Decode a base64 configuration string to TOML and exit.
    #[arg(long)]
    decode_config_base64: Option<String>,
    /// Validate configuration and server prerequisites, then exit.
    #[arg(long)]
    check_config: bool,
    /// Run deployment diagnostics and profile advice, then exit.
    #[arg(long)]
    doctor: bool,
    /// Check the configured release metadata endpoint and exit.
    #[arg(long)]
    check_update: bool,
    /// Override the release metadata URL used by --check-update.
    #[arg(long)]
    update_url: Option<String>,
    /// Override the tunnel listener address. Example: --listen 0.0.0.0:6690.
    #[arg(long)]
    listen: Option<SocketAddr>,
    /// Override the pre-shared key; also read from ESPEJISMO_PSK when set.
    #[arg(long, env = "ESPEJISMO_PSK")]
    psk: Option<String>,
    /// Override the allowed clock difference between peers, in seconds.
    #[arg(long)]
    clock_skew_secs: Option<i64>,
    /// Set the maximum data-frame padding in bytes.
    #[arg(long)]
    max_padding: Option<usize>,
    /// Set the maximum random frame delay in milliseconds.
    #[arg(long)]
    jitter_ms: Option<u64>,
    /// Set the chance of adding padding, from 0 to 100 percent.
    #[arg(long)]
    padding_chance_percent: Option<u8>,
    /// Set the backpressure detection threshold in milliseconds.
    #[arg(long)]
    backpressure_threshold_ms: Option<u64>,
    /// Set the delay before retrying after backpressure, in milliseconds.
    #[arg(long)]
    backpressure_cooldown_ms: Option<u64>,
    /// Set the handshake deadline in milliseconds.
    #[arg(long)]
    handshake_timeout_ms: Option<u64>,
    /// Set the delay before rejecting an unsuccessful handshake, in milliseconds.
    #[arg(long)]
    reject_delay_ms: Option<u64>,
    /// Set the maximum handshake padding in bytes.
    #[arg(long)]
    max_handshake_padding: Option<usize>,
    /// Set how long completed handshakes remain in the replay cache, in seconds.
    #[arg(long)]
    replay_window_secs: Option<i64>,
    /// Set the proof-of-work puzzle difficulty in bits.
    #[arg(long)]
    puzzle_bits: Option<u8>,
    /// Set the per-tunnel I/O buffer size in bytes.
    #[arg(long)]
    tunnel_buffer: Option<usize>,
    /// Set the delay before starting service after launch, in milliseconds.
    #[arg(long)]
    cold_start_delay_ms: Option<u64>,
    /// Set the maximum number of connections held in the tarpit.
    #[arg(long)]
    tarpit_max: Option<usize>,
    /// Set how long tarpit connections are held, in seconds.
    #[arg(long)]
    tarpit_hold_secs: Option<u64>,
    /// Set the logging filter, such as info, debug, or espejismo=trace.
    #[arg(long)]
    log_level: Option<String>,
    /// Select human-readable or JSON log output. Example: --log-format json.
    #[arg(long)]
    log_format: Option<String>,
    /// Append logs to this file. Example: --log-file ./server.log.
    #[arg(long)]
    log_file: Option<PathBuf>,
    /// Disable ANSI color codes in terminal log output.
    #[arg(long)]
    no_log_ansi: bool,
    /// Bind the admin API to this address. Example: --admin-listen 127.0.0.1:9090.
    #[arg(long)]
    admin_listen: Option<SocketAddr>,
    /// Set the bearer token required by the admin API.
    #[arg(long)]
    admin_token: Option<String>,
}

#[derive(Clone)]
pub(crate) struct RemoteRuntime {
    pub(crate) listen: SocketAddr,
    pub(crate) settings: Arc<RwLock<RemoteSettings>>,
    pub(crate) replay_window_secs: i64,
    pub(crate) tunnel_buffer: usize,
    pub(crate) tcp: TcpConfig,
    pub(crate) port_hopping: espejismo_core::PortHoppingConfig,
    pub(crate) tarpit_max: usize,
    pub(crate) tarpit_hold: Duration,
    pub(crate) admin_listen: Option<SocketAddr>,
    pub(crate) admin_token: Option<String>,
    pub(crate) reload_source: Option<ConfigInput>,
    pub(crate) reload_args: Args,
    pub(crate) runtime_state: RuntimeState,
    pub(crate) global_connection_limit: Arc<Semaphore>,
    pub(crate) global_stream_limit: Arc<Semaphore>,
}

#[derive(Clone)]
pub(crate) struct RemoteSettings {
    pub(crate) users: Arc<Vec<HandshakeUser>>,
    pub(crate) frames: FrameOptions,
    pub(crate) underlay: espejismo_core::UnderlayConfig,
    pub(crate) mux: espejismo_core::mux::MuxRuntimeConfig,
    pub(crate) handshake_timeout: Duration,
    pub(crate) reject_delay: Duration,
    pub(crate) cold_start_delay: Duration,
    pub(crate) fallback_http: FallbackHttpRuntime,
    pub(crate) egress: EgressPolicy,
    pub(crate) idle_timeout: Duration,
    pub(crate) max_streams: u32,
    pub(crate) limits: UserLimitRegistry,
    pub(crate) traffic: Arc<dyn TrafficObserver>,
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
        let mut example_config = parse_config(&example_config())?;
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
    if let Some(profile) = &args.profile {
        apply_named_profile(&mut config, profile)?;
    }
    apply_cli_overrides_to_config(&mut config, &args)?;
    if args.check_config || args.doctor {
        check_remote_config(&config, &args, args.doctor).await?;
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
    let runtime = build_runtime(config, &args, config_input)?;
    validate_admin_listener(runtime.admin_listen, runtime.listen)?;
    // Acquire every tunnel port before starting auxiliary tasks. This makes a
    // port-hopping bind failure an immediate startup error with its address.
    let listeners = bind_remote_listeners(&runtime)?;
    let metrics = Metrics::default();
    if let Some(addr) = runtime.admin_listen {
        let reload = runtime.reload_action();
        spawn_admin_server(
            addr,
            AdminState {
                role: "remote".to_string(),
                metrics: metrics.clone(),
                runtime: runtime.runtime_state.clone(),
                token: runtime.admin_token.clone(),
                reload,
            },
        );
    }
    let tarpit = tarpit::TarpitManager::spawn(runtime.tarpit_max, runtime.tarpit_hold);
    let replay = Arc::new(tokio::sync::Mutex::new(ReplayCache::new(
        runtime.replay_window_secs,
    )));
    let mux_mode = runtime.settings.read().await.mux.mode;
    info!(
        role = "remote",
        version = env!("CARGO_PKG_VERSION"),
        listen = %runtime.listen,
        mux = ?mux_mode,
        underlay = ?runtime.settings.read().await.underlay.mode,
        listeners = listeners.len(),
        "service started"
    );
    let (accepted_tx, mut accepted_rx) = mpsc::channel(1024);
    for listener in listeners {
        let accepted_tx = accepted_tx.clone();
        tokio::spawn(async move {
            let mut resource_failures = 0_u32;
            loop {
                match listener.accept().await {
                    Ok((socket, peer)) => {
                        resource_failures = 0;
                        if accepted_tx.send((socket, peer)).await.is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        if is_temporary_resource_exhaustion(&err) {
                            resource_failures = resource_failures.saturating_add(1);
                            let delay = accept_resource_retry_delay(resource_failures);
                            debug!(error = %err, retry_ms = delay.as_millis(), "remote listener temporarily out of resources; retrying accept");
                            tokio::time::sleep(delay).await;
                        } else {
                            debug!(error = %err, "remote listener accept failed");
                            break;
                        }
                    }
                }
            }
        });
    }
    drop(accepted_tx);

    loop {
        let accepted = tokio::select! {
            accepted = accepted_rx.recv() => accepted,
            _ = shutdown_signal() => {
                info!("shutdown signal received");
                break;
            }
        };
        let Some((socket, peer)) = accepted else {
            break;
        };
        let _ = apply_tcp_options(&socket, &runtime.tcp);
        let Some(connection_permit) = try_connection_permit(&runtime.global_connection_limit)
        else {
            debug!(%peer, "remote peer dropped because global connection limit is full");
            continue;
        };
        let replay = replay.clone();
        let runtime = runtime.clone();
        let tarpit = tarpit.clone();
        let metrics = metrics.clone();
        metrics.inc_accepted();
        tokio::spawn(async move {
            let _connection_permit = connection_permit;
            if let Err(err) = handle_peer(socket, runtime, replay, tarpit, metrics).await {
                debug!(%peer, error = %err, "remote peer ended");
            }
        });
    }
    Ok(())
}

fn accept_resource_retry_delay(consecutive_failures: u32) -> Duration {
    if consecutive_failures == 0 {
        return Duration::ZERO;
    }
    let exponent = consecutive_failures.saturating_sub(1).min(6);
    ACCEPT_RESOURCE_RETRY_BASE_DELAY
        .saturating_mul(1_u32 << exponent)
        .min(ACCEPT_RESOURCE_RETRY_MAX_DELAY)
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

fn is_temporary_resource_exhaustion(err: &std::io::Error) -> bool {
    if err.kind() == std::io::ErrorKind::OutOfMemory {
        return true;
    }

    match err.raw_os_error() {
        // Linux: ENFILE, EMFILE, ENOMEM. Windows: ERROR_NOT_ENOUGH_MEMORY,
        // ERROR_OUTOFMEMORY. Tokio reports these as OS errors on accept.
        #[cfg(target_os = "linux")]
        Some(23) | Some(24) | Some(12) => true,
        #[cfg(windows)]
        Some(8) | Some(14) => true,
        _ => false,
    }
}

fn try_connection_permit(limit: &Arc<Semaphore>) -> Option<tokio::sync::OwnedSemaphorePermit> {
    limit.clone().try_acquire_owned().ok()
}

fn log_overrides(args: &Args) -> LogOverrides {
    LogOverrides {
        level: args.log_level.clone(),
        format: args.log_format.clone(),
        file: args.log_file.clone(),
        no_ansi: args.no_log_ansi,
    }
}

fn bind_remote_listeners(runtime: &RemoteRuntime) -> Result<Vec<tokio::net::TcpListener>> {
    let mut addrs = Vec::new();
    addrs.push(runtime.listen);
    if runtime.port_hopping.enabled {
        for &port in &runtime.port_hopping.ports {
            let mut addr = runtime.listen;
            addr.set_port(port);
            if !addrs.contains(&addr) {
                addrs.push(addr);
            }
        }
    }
    addrs
        .into_iter()
        .map(|addr| bind_tcp_listener(addr, &runtime.tcp))
        .collect()
}

fn validate_admin_listener(admin: Option<SocketAddr>, remote: SocketAddr) -> Result<()> {
    anyhow::ensure!(
        admin != Some(remote),
        "admin.listen must not reuse remote.listen"
    );
    Ok(())
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
    if let Some(value) = args.listen {
        config.remote.listen = value;
    }
    if let Some(value) = args.handshake_timeout_ms {
        config.remote.handshake_timeout_ms = value;
    }
    if let Some(value) = args.reject_delay_ms {
        config.remote.reject_delay_ms = value;
    }
    if let Some(value) = args.max_handshake_padding {
        config.remote.max_handshake_padding = value;
    }
    if let Some(value) = args.replay_window_secs {
        config.remote.replay_window_secs = value;
    }
    if let Some(value) = args.cold_start_delay_ms {
        config.remote.cold_start_delay_ms = value;
    }
    if let Some(value) = args.tarpit_max {
        config.remote.tarpit_max = value;
    }
    if let Some(value) = args.tarpit_hold_secs {
        config.remote.tarpit_hold_secs = value;
    }
    if let Some(value) = args.admin_listen {
        config.admin.listen = Some(value);
    }
    if let Some(value) = &args.admin_token {
        config.admin.token = Some(value.clone());
    }
    apply_log_overrides(&mut config.logging, &log_overrides(args))?;
    let normalized = config_to_toml(config)?;
    *config = parse_config(&normalized)?;
    Ok(())
}

async fn check_remote_config(config: &EspejismoConfig, args: &Args, doctor: bool) -> Result<()> {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let listen = args.listen.unwrap_or(config.remote.listen);
    let admin_addr = args.admin_listen.or(config.admin.listen);
    if admin_addr == Some(listen) {
        errors.push("admin.listen must not reuse remote.listen".to_string());
    }
    match bind_tcp_listener(listen, &config.shared.tcp) {
        Ok(listener) => {
            drop(listener);
            println!("OK remote.listen can bind: {listen}");
        }
        Err(err) => errors.push(format!("remote.listen cannot bind {listen}: {err}")),
    }
    if let Some(addr) = admin_addr {
        match bind_tcp_listener(addr, &config.shared.tcp) {
            Ok(listener) => {
                drop(listener);
                println!("OK admin.listen can bind: {addr}");
            }
            Err(err) => errors.push(format!("admin.listen cannot bind {addr}: {err}")),
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
    if config.remote.users.is_empty() {
        if let Some(psk) = args.psk.as_ref().or(config.shared.psk.as_ref()) {
            if psk.len() < 16 {
                warnings.push(
                    "PSK is shorter than 16 characters; use a longer random secret".to_string(),
                );
            } else {
                println!("OK single-key PSK length looks usable");
            }
        } else {
            errors.push("remote.users or shared.psk/--psk/ESPEJISMO_PSK is required".to_string());
        }
    } else {
        println!("OK {} remote user(s) configured", config.remote.users.len());
    }
    if !config.remote.egress.deny_private_ips
        && config.remote.egress.allow_hosts.is_empty()
        && config.remote.egress.allow_ports.is_empty()
    {
        warnings.push(
            "egress policy is broad; consider deny_private_ips or explicit allow lists".to_string(),
        );
    }
    if let Some(proxy) = EgressPolicy::from(config.remote.egress.clone()).upstream_proxy()? {
        match espejismo_core::resolve_socket_addrs(proxy.endpoint.as_str()).await {
            Ok(addrs) => {
                if !addrs.is_empty() {
                    println!("OK egress proxy resolves: {}", proxy.endpoint);
                } else {
                    warnings.push(format!(
                        "egress proxy resolved no addresses: {}",
                        proxy.endpoint
                    ));
                }
            }
            Err(err) => warnings.push(format!(
                "egress proxy cannot resolve {}: {err}",
                proxy.endpoint
            )),
        }
    }
    if doctor {
        diagnose_low_feature_profile(config, &mut warnings);
        if let Some(upstream) = &config.remote.fallback_http.upstream {
            let host = upstream
                .strip_prefix("http://")
                .or_else(|| upstream.strip_prefix("https://"))
                .unwrap_or(upstream)
                .split('/')
                .next()
                .unwrap_or(upstream);
            match espejismo_core::resolve_socket_addrs(host).await {
                Ok(addrs) => {
                    if !addrs.is_empty() {
                        println!("OK fallback upstream resolves: {upstream}");
                    } else {
                        warnings.push(format!(
                            "fallback upstream resolved no addresses: {upstream}"
                        ));
                    }
                }
                Err(err) => warnings.push(format!(
                    "fallback upstream cannot resolve {upstream}: {err}"
                )),
            }
        }
        if config.shared.obfuscation.profile == espejismo_core::ObfuscationProfile::Stealth {
            println!(
                "OK stealth frame size configured: {} bytes",
                config.shared.stealth.frame_size
            );
        }
        if matches!(
            config.remote.fallback_http.mode,
            ProbeDefenseMode::HttpFallback
        ) || config.remote.fallback_http.enabled
        {
            warnings.push(
                "low-feature mode: HTTP fallback is useful operationally but borrows HTTP-looking behavior"
                    .to_string(),
            );
        }
    }
    report_config_check(warnings, errors)
}

fn diagnose_low_feature_profile(config: &EspejismoConfig, warnings: &mut Vec<String>) {
    if !config.shared.obfuscation.profile.is_stealth() {
        warnings.push(
            "low-feature mode: use profile stealth to avoid variable frame-size patterns"
                .to_string(),
        );
    }
    if config.shared.max_padding == 0 || config.shared.padding_chance_percent == 0 {
        warnings.push(
            "low-feature mode: enable bounded padding to reduce stable payload-size signals"
                .to_string(),
        );
    }
    if config.shared.jitter_ms == 0 && !config.shared.obfuscation.profile.is_stealth() {
        warnings.push(
            "low-feature mode: non-stealth profile without jitter keeps timing more regular"
                .to_string(),
        );
    }
    if config.shared.key_update_frames > 100_000 {
        warnings.push(
            "low-feature mode: very infrequent key updates keep long tunnels on one traffic secret"
                .to_string(),
        );
    }
    if config.shared.tcp.heartbeat_secs > 0 && !config.shared.obfuscation.profile.is_stealth() {
        warnings.push(
            "low-feature mode: regular non-stealth heartbeats can become a timing signal"
                .to_string(),
        );
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
                .with_stealth_frame_size(stealth_handshake)
                .with_handshake_window(config.shared.handshake_window.into())
                .with_mux_mode(config.shared.mux.mode),
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
        .with_stealth_frame_size(stealth_handshake)
        .with_handshake_window(config.shared.handshake_window.into())
        .with_mux_mode(config.shared.mux.mode),
    });
    Ok(users)
}

fn build_runtime(
    config: EspejismoConfig,
    args: &Args,
    reload_source: ConfigInput,
) -> Result<RemoteRuntime> {
    let settings = build_remote_settings(&config, args)?;
    let connection_limit = connection_limit_capacity(&config);
    Ok(RemoteRuntime {
        listen: args.listen.unwrap_or(config.remote.listen),
        settings: Arc::new(RwLock::new(settings)),
        replay_window_secs: args
            .replay_window_secs
            .unwrap_or(config.remote.replay_window_secs),
        tunnel_buffer: args.tunnel_buffer.unwrap_or(config.shared.tunnel_buffer),
        tcp: config.shared.tcp.clone(),
        port_hopping: config.shared.port_hopping.clone(),
        tarpit_max: args.tarpit_max.unwrap_or(config.remote.tarpit_max),
        tarpit_hold: Duration::from_secs(
            args.tarpit_hold_secs
                .unwrap_or(config.remote.tarpit_hold_secs),
        ),
        admin_listen: args.admin_listen.or(config.admin.listen),
        admin_token: args.admin_token.clone().or(config.admin.token),
        reload_source: (reload_source.path.is_some() || reload_source.base64.is_some())
            .then_some(reload_source),
        reload_args: sanitized_reload_args(args),
        runtime_state: RuntimeState::default(),
        global_connection_limit: Arc::new(Semaphore::new(connection_limit)),
        global_stream_limit: Arc::new(Semaphore::new(config.shared.max_streams.max(1) as usize)),
    })
}

fn connection_limit_capacity(config: &EspejismoConfig) -> usize {
    config.shared.max_physical_connections.max(1) as usize
}

fn sanitized_reload_args(args: &Args) -> Args {
    let mut sanitized = args.clone();
    sanitized.psk = None;
    sanitized.admin_token = None;
    sanitized
}

fn build_remote_settings(config: &EspejismoConfig, args: &Args) -> Result<RemoteSettings> {
    let stealth_frame_size = config.shared.stealth.frame_size;
    let obfuscation_profile = config.shared.obfuscation.profile;
    let stealth_handshake = obfuscation_profile
        .is_stealth()
        .then_some(stealth_frame_size);
    let limits = build_user_limits(config);

    Ok(RemoteSettings {
        users: Arc::new(build_handshake_users(config, args, stealth_handshake)?),
        frames: config.shared.frame_options(&FrameOptionOverrides {
            max_padding: args.max_padding,
            jitter_ms: args.jitter_ms,
            padding_chance_percent: args.padding_chance_percent,
            backpressure_threshold_ms: args.backpressure_threshold_ms,
            backpressure_cooldown_ms: args.backpressure_cooldown_ms,
        }),
        underlay: config.shared.underlay.clone(),
        mux: espejismo_core::mux::MuxRuntimeConfig::from_config(
            config.shared.max_streams,
            &config.shared.mux,
        ),
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
        traffic: Arc::new(NoopTrafficObserver),
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
        let runtime_state = self.runtime_state.clone();
        let action: AdminAction = Arc::new(move |body: Option<String>| {
            let source = source.clone();
            let settings = settings.clone();
            let args = args.clone();
            let runtime_state = runtime_state.clone();
            Box::pin(async move {
                let mut config = if let Some(body) = body {
                    parse_config(&body)?
                } else {
                    load_config(
                        source
                            .context("reload requires --config or --config-base64; use /apply")?,
                    )?
                };
                apply_log_overrides(&mut config.logging, &log_overrides(&args))?;
                let next = build_remote_settings(&config, &args)?;
                let user_count = replace_remote_settings(&settings, next).await;
                runtime_state.mark_config_applied();
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

// Build and validate the complete candidate before entering this commit point.
// A failed parse/build therefore cannot expose a mixture of old and new policy.
async fn replace_remote_settings(settings: &RwLock<RemoteSettings>, next: RemoteSettings) -> usize {
    let user_count = next.users.len();
    *settings.write().await = next;
    user_count
}

#[cfg(test)]
mod connection_limit_tests {
    use super::{connection_limit_capacity, try_connection_permit};
    use espejismo_core::{config::example_config, parse_config};
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    #[test]
    fn configured_physical_connection_limit_is_enforced_until_permit_is_released() {
        let mut config = parse_config(&example_config()).expect("example config parses");
        config.shared.max_physical_connections = 2;
        let limit = Arc::new(Semaphore::new(connection_limit_capacity(&config)));

        let first = try_connection_permit(&limit).expect("first connection admitted");
        let second = try_connection_permit(&limit).expect("second connection admitted");
        assert!(
            try_connection_permit(&limit).is_none(),
            "excess connection rejected"
        );
        assert_eq!(limit.available_permits(), 0);

        drop(first);
        assert!(
            try_connection_permit(&limit).is_some(),
            "released capacity is reusable"
        );
        drop(second);
        assert_eq!(limit.available_permits(), 2);
    }
}

#[cfg(test)]
mod accept_resource_tests {
    use super::{accept_resource_retry_delay, is_temporary_resource_exhaustion};
    use std::time::Duration;

    #[test]
    fn retry_delay_grows_and_stays_bounded() {
        assert_eq!(accept_resource_retry_delay(0), Duration::ZERO);
        assert_eq!(accept_resource_retry_delay(1), Duration::from_millis(250));
        assert_eq!(accept_resource_retry_delay(2), Duration::from_millis(500));
        assert_eq!(accept_resource_retry_delay(3), Duration::from_millis(1_000));
        assert_eq!(
            accept_resource_retry_delay(7),
            Duration::from_millis(16_000)
        );
        assert_eq!(
            accept_resource_retry_delay(u32::MAX),
            Duration::from_millis(16_000)
        );
    }

    #[test]
    fn retry_delay_is_monotonic_and_remains_capped_after_saturation() {
        let mut previous = Duration::ZERO;
        for failures in 0..=32 {
            let delay = accept_resource_retry_delay(failures);
            assert!(delay >= previous, "delay decreased at failure {failures}");
            assert!(delay <= Duration::from_secs(16));
            previous = delay;
        }
        assert_eq!(accept_resource_retry_delay(7), Duration::from_secs(16));
        assert_eq!(accept_resource_retry_delay(32), Duration::from_secs(16));
    }

    #[test]
    fn retries_known_descriptor_and_memory_exhaustion_errors() {
        #[cfg(target_os = "linux")]
        for code in [12, 23, 24] {
            assert!(is_temporary_resource_exhaustion(
                &std::io::Error::from_raw_os_error(code)
            ));
        }
        #[cfg(windows)]
        for code in [8, 14] {
            assert!(is_temporary_resource_exhaustion(
                &std::io::Error::from_raw_os_error(code)
            ));
        }
        assert!(is_temporary_resource_exhaustion(&std::io::Error::from(
            std::io::ErrorKind::OutOfMemory
        )));
    }

    #[test]
    fn does_not_retry_permanent_accept_errors() {
        assert!(!is_temporary_resource_exhaustion(&std::io::Error::from(
            std::io::ErrorKind::PermissionDenied
        )));
        assert!(!is_temporary_resource_exhaustion(&std::io::Error::from(
            std::io::ErrorKind::ConnectionAborted
        )));
    }
}

#[cfg(test)]
mod startup_validation_tests {
    use super::validate_admin_listener;
    use std::net::SocketAddr;

    #[test]
    fn rejects_admin_listener_reusing_remote_address() {
        let addr: SocketAddr = "127.0.0.1:6690".parse().unwrap();
        assert!(validate_admin_listener(Some(addr), addr).is_err());
        assert!(validate_admin_listener(None, addr).is_ok());
        assert!(validate_admin_listener(Some("127.0.0.1:9090".parse().unwrap()), addr).is_ok());
    }
}

#[cfg(test)]
mod reload_safety_tests {
    use super::{build_remote_settings, replace_remote_settings, Args};
    use clap::Parser;
    use espejismo_core::{config::example_config, parse_config};
    use tokio::sync::RwLock;

    fn test_args() -> Args {
        Args::try_parse_from(["espejismo-remote"]).expect("default arguments parse")
    }

    #[tokio::test]
    async fn reload_candidate_is_committed_as_one_complete_settings_value() {
        let old_config = parse_config(&example_config()).expect("example config parses");
        let old = build_remote_settings(&old_config, &test_args()).expect("initial settings build");
        let settings = RwLock::new(old);

        let mut next_config = old_config;
        next_config.remote.users.pop();
        let next =
            build_remote_settings(&next_config, &test_args()).expect("replacement settings build");
        let expected_users = next.users.len();

        let committed_users = replace_remote_settings(&settings, next).await;
        let committed = settings.read().await;
        assert_eq!(committed_users, expected_users);
        assert_eq!(committed.users.len(), expected_users);
    }

    #[tokio::test]
    async fn invalid_candidate_does_not_replace_current_settings() {
        let config = parse_config(&example_config()).expect("example config parses");
        let initial = build_remote_settings(&config, &test_args()).expect("initial settings build");
        let original_user_count = initial.users.len();
        let settings = RwLock::new(initial);

        let candidate = build_remote_settings(
            &config,
            &Args::try_parse_from(["espejismo-remote", "--psk", "x"])
                .expect("invalid candidate arguments parse"),
        );
        assert!(candidate.is_err(), "invalid PSK must fail settings build");

        let current = settings.read().await;
        assert_eq!(current.users.len(), original_user_count);
    }
}
