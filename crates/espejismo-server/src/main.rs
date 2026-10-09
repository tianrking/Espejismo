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
const PEER_SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

mod fallback;
mod handler;
mod http_chain;
mod limits;
mod mux;
mod proxy_protocol;
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
    let reload = runtime.reload_action();
    if let Some(addr) = runtime.admin_listen {
        spawn_admin_server(
            addr,
            AdminState {
                role: "remote".to_string(),
                metrics: metrics.clone(),
                runtime: runtime.runtime_state.clone(),
                token: runtime.admin_token.clone(),
                reload: reload.clone(),
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

    let mut sighup = register_sighup()?;

    let mut peer_tasks = tokio::task::JoinSet::new();
    loop {
        let accepted = tokio::select! {
            accepted = accepted_rx.recv() => accepted,
            _ = wait_for_sighup(&mut sighup) => {
                match run_signal_reload(reload.as_ref()).await {
                    Some(Ok(_)) => info!("configuration reloaded after SIGHUP"),
                    Some(Err(err)) => debug!(error = %err, "configuration reload after SIGHUP failed"),
                    None => debug!("SIGHUP ignored because configuration reload is unavailable"),
                }
                continue;
            }
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
        peer_tasks.spawn(async move {
            let _connection_permit = connection_permit;
            if let Err(err) = handle_peer(socket, runtime, replay, tarpit, metrics).await {
                debug!(%peer, error = %err, "remote peer ended");
            }
        });
    }
    // Stop accepting first, then give established tunnels a bounded chance to
    // finish before the process exits. This is connection draining, not
    // cross-process migration: unfinished streams are still interrupted.
    drain_peer_tasks(&mut peer_tasks, PEER_SHUTDOWN_GRACE).await;
    Ok(())
}

async fn drain_peer_tasks(tasks: &mut tokio::task::JoinSet<()>, grace: Duration) {
    if tokio::time::timeout(grace, async { while tasks.join_next().await.is_some() {} })
        .await
        .is_err()
    {
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
}

#[cfg(unix)]
type SighupListener = tokio::signal::unix::Signal;
#[cfg(not(unix))]
struct SighupListener;

fn register_sighup() -> Result<SighupListener> {
    #[cfg(unix)]
    {
        Ok(
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
                .context("register SIGHUP handler")?,
        )
    }
    #[cfg(not(unix))]
    {
        Ok(SighupListener)
    }
}

async fn wait_for_sighup(listener: &mut SighupListener) {
    #[cfg(unix)]
    {
        listener.recv().await;
    }
    #[cfg(not(unix))]
    {
        let _ = listener;
        std::future::pending::<()>().await;
    }
}

async fn run_signal_reload(
    reload: Option<&AdminAction>,
) -> Option<anyhow::Result<serde_json::Value>> {
    match reload {
        Some(action) => Some(action(None).await),
        None => None,
    }
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

    #[test]
    fn physical_connection_limit_capacity_handles_configuration_boundaries() {
        let mut config = parse_config(&example_config()).expect("example config parses");

        config.shared.max_physical_connections = 0;
        assert_eq!(connection_limit_capacity(&config), 1);

        config.shared.max_physical_connections = 1;
        assert_eq!(connection_limit_capacity(&config), 1);

        config.shared.max_physical_connections = 65_535;
        assert_eq!(connection_limit_capacity(&config), 65_535);
    }

    #[test]
    fn single_connection_limit_rejects_until_the_only_permit_is_released() {
        let limit = Arc::new(Semaphore::new(1));

        let permit = try_connection_permit(&limit).expect("first connection admitted");
        assert!(
            try_connection_permit(&limit).is_none(),
            "second connection rejected"
        );
        assert_eq!(limit.available_permits(), 0);

        drop(permit);
        assert_eq!(limit.available_permits(), 1);
        assert!(
            try_connection_permit(&limit).is_some(),
            "capacity is reusable"
        );
    }

    #[tokio::test]
    async fn concurrent_connection_attempts_never_exceed_capacity() {
        const CAPACITY: usize = 8;
        const ATTEMPTS: usize = 64;

        let limit = Arc::new(Semaphore::new(CAPACITY));
        let barrier = Arc::new(tokio::sync::Barrier::new(ATTEMPTS));
        let mut tasks = tokio::task::JoinSet::new();

        for _ in 0..ATTEMPTS {
            let limit = Arc::clone(&limit);
            let barrier = Arc::clone(&barrier);
            tasks.spawn(async move {
                barrier.wait().await;
                try_connection_permit(&limit)
            });
        }

        let mut admitted = 0;
        let mut rejected = 0;
        let mut permits = Vec::new();
        while let Some(result) = tasks.join_next().await {
            match result.expect("connection attempt task completes") {
                Some(permit) => {
                    admitted += 1;
                    permits.push(permit);
                }
                None => rejected += 1,
            }
        }

        assert_eq!(admitted, CAPACITY, "only available slots are admitted");
        assert_eq!(rejected, ATTEMPTS - CAPACITY, "overflow is rejected");
        assert_eq!(limit.available_permits(), 0, "admitted permits are counted");

        drop(permits);
        assert_eq!(limit.available_permits(), CAPACITY);
        assert!(try_connection_permit(&limit).is_some());
    }
}

#[cfg(test)]
mod restart_drain_tests {
    use super::drain_peer_tasks;
    use std::time::Duration;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn shutdown_waits_for_an_existing_peer_to_finish() {
        let (finish_tx, finish_rx) = oneshot::channel::<()>();
        let mut tasks = tokio::task::JoinSet::new();
        tasks.spawn(async move {
            let _ = finish_rx.await;
        });

        finish_tx.send(()).unwrap();
        drain_peer_tasks(&mut tasks, Duration::from_secs(1)).await;
        assert!(
            tasks.is_empty(),
            "existing peer task must finish before exit"
        );
    }

    #[tokio::test]
    async fn shutdown_aborts_peer_tasks_after_grace_expires() {
        let mut tasks = tokio::task::JoinSet::new();
        tasks.spawn(std::future::pending::<()>());
        drain_peer_tasks(&mut tasks, Duration::from_millis(1)).await;
        assert!(tasks.is_empty(), "overdue peer task must be aborted");
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
    use super::{build_remote_settings, replace_remote_settings, run_signal_reload, Args};
    use clap::Parser;
    use espejismo_core::{config::example_config, parse_config, AdminAction};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    async fn handshake_with_current_settings(
        settings: &RwLock<super::RemoteSettings>,
        client_config: espejismo_core::HandshakeConfig,
    ) -> (
        anyhow::Result<espejismo_core::AuthenticatedSession>,
        anyhow::Result<espejismo_core::SessionKeys>,
    ) {
        use espejismo_core::{accept_handshake_with_users, connect_handshake, ReplayCache};
        use std::sync::Arc;
        use tokio::io::duplex;
        use tokio::sync::Mutex;

        let users = settings.read().await.users.clone();
        let (mut client, mut server) = duplex(4096);
        let replay = Arc::new(Mutex::new(ReplayCache::new(60)));
        let server_task = tokio::spawn(async move {
            accept_handshake_with_users(&mut server, &users, replay).await
        });
        let client_result = connect_handshake(&mut client, &client_config).await;
        let server_result = server_task.await.expect("server handshake task joins");
        (server_result, client_result)
    }

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
        next_config.shared.idle_timeout_secs += 17;
        next_config.shared.max_streams += 3;
        next_config.remote.egress.proxy = Some("https://rotated-proxy.example:8443".to_string());
        let next =
            build_remote_settings(&next_config, &test_args()).expect("replacement settings build");
        let expected_users = next.users.len();
        let expected_idle_timeout = next.idle_timeout;
        let expected_max_streams = next.max_streams;
        let expected_proxy = next
            .egress
            .upstream_proxy()
            .expect("replacement proxy config is valid")
            .expect("replacement HTTPS proxy is configured");

        let committed_users = replace_remote_settings(&settings, next).await;
        let committed = settings.read().await;
        assert_eq!(committed_users, expected_users);
        assert_eq!(committed.users.len(), expected_users);
        assert_eq!(committed.idle_timeout, expected_idle_timeout);
        assert_eq!(committed.max_streams, expected_max_streams);
        assert_eq!(
            committed
                .egress
                .upstream_proxy()
                .expect("committed proxy config is valid")
                .expect("committed HTTPS proxy is configured"),
            expected_proxy
        );
    }

    #[tokio::test]
    async fn runtime_psk_rotation_rejects_old_key_and_accepts_new_key() {
        let mut old_config = parse_config(&example_config()).expect("example config parses");
        old_config.remote.users.clear();
        old_config.shared.psk = Some("old-runtime-rotation-key-123456".to_string());
        let old = build_remote_settings(&old_config, &test_args()).expect("initial settings build");
        let old_client = old.users[0].config.clone();
        let settings = RwLock::new(old);

        let mut rotated_config = old_config;
        rotated_config.shared.psk = Some("new-runtime-rotation-key-abcdef".to_string());
        let rotated =
            build_remote_settings(&rotated_config, &test_args()).expect("rotated settings build");
        let new_client = rotated.users[0].config.clone();
        replace_remote_settings(&settings, rotated).await;

        let (old_server, old_client_result) =
            handshake_with_current_settings(&settings, old_client).await;
        assert!(old_server.is_err(), "retired PSK must not authenticate");
        assert!(
            old_client_result.is_err(),
            "retired PSK must not complete handshake"
        );

        let (new_server, new_client_result) =
            handshake_with_current_settings(&settings, new_client).await;
        assert_eq!(new_server.expect("new PSK authenticates").user, "default");
        assert!(new_client_result.is_ok(), "new PSK completes handshake");
    }

    #[tokio::test]
    async fn invalid_candidate_does_not_replace_current_settings() {
        let config = parse_config(&example_config()).expect("example config parses");
        let initial = build_remote_settings(&config, &test_args()).expect("initial settings build");
        let original_user_count = initial.users.len();
        let original_idle_timeout = initial.idle_timeout;
        let original_max_streams = initial.max_streams;
        let original_proxy = initial.egress.upstream_proxy().unwrap();
        let settings = RwLock::new(initial);

        let candidate = build_remote_settings(
            &config,
            &Args::try_parse_from(["espejismo-remote", "--psk", "x"])
                .expect("invalid candidate arguments parse"),
        );
        assert!(candidate.is_err(), "invalid PSK must fail settings build");

        let current = settings.read().await;
        assert_eq!(current.users.len(), original_user_count);
        assert_eq!(current.idle_timeout, original_idle_timeout);
        assert_eq!(current.max_streams, original_max_streams);
        assert_eq!(current.egress.upstream_proxy().unwrap(), original_proxy);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn signal_reload_uses_atomic_candidate_commit() {
        let config = parse_config(&example_config()).expect("example config parses");
        let initial = build_remote_settings(&config, &test_args()).expect("initial settings build");
        let original_idle_timeout = initial.idle_timeout;
        let settings = Arc::new(RwLock::new(initial));
        let mut next_config = config.clone();
        next_config.shared.idle_timeout_secs += 17;
        let success_settings = settings.clone();
        let success: AdminAction = Arc::new(move |_| {
            let settings = success_settings.clone();
            let config = next_config.clone();
            Box::pin(async move {
                let next = build_remote_settings(&config, &test_args())?;
                replace_remote_settings(&settings, next).await;
                Ok(json!({"applied": true}))
            })
        });
        assert!(run_signal_reload(Some(&success)).await.unwrap().is_ok());
        let applied_timeout = settings.read().await.idle_timeout;
        assert_ne!(applied_timeout, original_idle_timeout);

        let failure_settings = settings.clone();
        let failure: AdminAction = Arc::new(move |_| {
            let settings = failure_settings.clone();
            let config = config.clone();
            Box::pin(async move {
                let invalid_args = Args::try_parse_from(["espejismo-remote", "--psk", "x"])?;
                let next = build_remote_settings(&config, &invalid_args)?;
                replace_remote_settings(&settings, next).await;
                Ok(json!({"applied": true}))
            })
        });
        assert!(run_signal_reload(Some(&failure)).await.unwrap().is_err());
        assert_eq!(settings.read().await.idle_timeout, applied_timeout);
    }
}
