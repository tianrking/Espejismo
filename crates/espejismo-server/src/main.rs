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
    HandshakeConfig, HandshakeUser, LogOverrides, Metrics, ProbeDefenseMode, ReplayCache,
    RuntimeState, TcpConfig,
};
use serde_json::json;
use tokio::net::lookup_host;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info};

mod fallback;
mod handler;
mod limits;
mod mux;
mod relay;
mod socks5_chain;
mod tarpit;

use fallback::FallbackHttpRuntime;
use handler::handle_peer;
use limits::{UserLimitConfig, UserLimitRegistry};

#[derive(Parser, Debug, Clone)]
#[command(name = "espejismo-remote", version)]
pub(crate) struct Args {
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
pub(crate) struct RemoteRuntime {
    pub(crate) listen: SocketAddr,
    pub(crate) settings: Arc<RwLock<RemoteSettings>>,
    pub(crate) replay_window_secs: i64,
    pub(crate) tunnel_buffer: usize,
    pub(crate) tcp: TcpConfig,
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
    pub(crate) mux: espejismo_core::mux::MuxRuntimeConfig,
    pub(crate) handshake_timeout: Duration,
    pub(crate) reject_delay: Duration,
    pub(crate) cold_start_delay: Duration,
    pub(crate) fallback_http: FallbackHttpRuntime,
    pub(crate) egress: EgressPolicy,
    pub(crate) idle_timeout: Duration,
    pub(crate) max_streams: u32,
    pub(crate) limits: UserLimitRegistry,
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
    if args.check_config {
        check_remote_config(&config, &args).await?;
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

    let listener = bind_tcp_listener(runtime.listen, &runtime.tcp)?;
    let tarpit = tarpit::TarpitManager::spawn(runtime.tarpit_max, runtime.tarpit_hold);
    let replay = Arc::new(tokio::sync::Mutex::new(ReplayCache::new(
        runtime.replay_window_secs,
    )));
    let mux_mode = runtime.settings.read().await.mux.mode;
    info!(listen = %runtime.listen, mux = ?mux_mode, "remote listening with mux tunnel support");

    loop {
        let (socket, peer) = listener.accept().await?;
        let _ = apply_tcp_options(&socket, &runtime.tcp);
        let Ok(connection_permit) = runtime.global_connection_limit.clone().try_acquire_owned()
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
}

fn log_overrides(args: &Args) -> LogOverrides {
    LogOverrides {
        level: args.log_level.clone(),
        format: args.log_format.clone(),
        file: args.log_file.clone(),
        no_ansi: args.no_log_ansi,
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

async fn check_remote_config(config: &EspejismoConfig, args: &Args) -> Result<()> {
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
    if let Some(proxy) = &config.remote.egress.socks5_proxy {
        match lookup_host(proxy.as_str()).await {
            Ok(addrs) => {
                if addrs.count() > 0 {
                    println!("OK egress SOCKS5 proxy resolves: {proxy}");
                } else {
                    warnings.push(format!(
                        "egress SOCKS5 proxy resolved no addresses: {proxy}"
                    ));
                }
            }
            Err(err) => warnings.push(format!("egress SOCKS5 proxy cannot resolve {proxy}: {err}")),
        }
    }
    report_config_check(warnings, errors)
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
    Ok(RemoteRuntime {
        listen: args.listen.unwrap_or(config.remote.listen),
        settings: Arc::new(RwLock::new(settings)),
        replay_window_secs: args
            .replay_window_secs
            .unwrap_or(config.remote.replay_window_secs),
        tunnel_buffer: args.tunnel_buffer.unwrap_or(config.shared.tunnel_buffer),
        tcp: config.shared.tcp.clone(),
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
        global_connection_limit: Arc::new(Semaphore::new(
            config.shared.max_physical_connections.max(1) as usize,
        )),
        global_stream_limit: Arc::new(Semaphore::new(config.shared.max_streams.max(1) as usize)),
    })
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
                let user_count = next.users.len();
                *settings.write().await = next;
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
