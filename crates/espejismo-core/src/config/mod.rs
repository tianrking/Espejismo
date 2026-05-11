use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::path::PathBuf;

use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::egress::EgressPolicy;
use crate::ingress::ProxyAuth;
use crate::protocol::framing::{
    ObfuscationProfile, DEFAULT_STEALTH_FRAME_SIZE, DEFAULT_STEALTH_TICK_MS,
};

#[derive(Clone, Debug, Default)]
pub struct ConfigInput {
    pub path: Option<String>,
    pub base64: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EspejismoConfig {
    #[serde(default)]
    pub shared: SharedConfig,
    #[serde(default)]
    pub local: LocalConfig,
    #[serde(default)]
    pub remote: RemoteConfig,
    #[serde(default)]
    pub logging: LogConfig,
    #[serde(default)]
    pub admin: AdminConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedConfig {
    pub psk: Option<String>,
    #[serde(default = "default_clock_skew_secs")]
    pub clock_skew_secs: i64,
    #[serde(default = "default_puzzle_bits")]
    pub puzzle_bits: u8,
    #[serde(default = "default_max_padding")]
    pub max_padding: usize,
    #[serde(default)]
    pub jitter_ms: u64,
    #[serde(default = "default_padding_chance_percent")]
    pub padding_chance_percent: u8,
    #[serde(default = "default_backpressure_threshold_ms")]
    pub backpressure_threshold_ms: u64,
    #[serde(default = "default_backpressure_cooldown_ms")]
    pub backpressure_cooldown_ms: u64,
    #[serde(default = "default_tunnel_buffer")]
    pub tunnel_buffer: usize,
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_max_streams")]
    pub max_streams: u32,
    #[serde(default)]
    pub tcp: TcpConfig,
    #[serde(default)]
    pub pacing: PacingConfig,
    #[serde(default)]
    pub obfuscation: ObfuscationConfig,
    #[serde(default)]
    pub stealth: StealthConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TcpConfig {
    #[serde(default = "default_tcp_nodelay")]
    pub nodelay: bool,
    #[serde(default = "default_tcp_keepalive_secs")]
    pub keepalive_secs: u64,
    #[serde(default)]
    pub user_timeout_ms: u64,
    #[serde(default)]
    pub send_buffer_bytes: usize,
    #[serde(default)]
    pub recv_buffer_bytes: usize,
    #[serde(default)]
    pub congestion_control: Option<String>,
    #[serde(default = "default_tcp_heartbeat_secs")]
    pub heartbeat_secs: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacingConfig {
    #[serde(default = "default_pacing_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub max_bytes_per_sec: u64,
    #[serde(default = "default_pacing_burst_bytes")]
    pub burst_bytes: usize,
    #[serde(default = "default_pacing_min_write_bytes")]
    pub min_write_bytes: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObfuscationConfig {
    #[serde(default)]
    pub profile: ObfuscationProfile,
    #[serde(default = "default_randomize_chunks")]
    pub randomize_chunks: bool,
    #[serde(default = "default_min_chunk")]
    pub min_chunk: usize,
    #[serde(default = "default_max_chunk")]
    pub max_chunk: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StealthConfig {
    #[serde(default = "default_stealth_frame_size")]
    pub frame_size: usize,
    #[serde(default = "default_stealth_tick_ms")]
    pub tick_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalConfig {
    pub server: Option<String>,
    #[serde(default = "default_socks5_listen")]
    pub socks5_listen: Option<SocketAddr>,
    #[serde(default = "default_http_listen")]
    pub http_listen: Option<SocketAddr>,
    #[serde(default = "default_handshake_padding")]
    pub handshake_padding: usize,
    #[serde(default)]
    pub auth: Option<ProxyAuth>,
    #[serde(default)]
    pub tun: LocalTunConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalTunConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_tun_name")]
    pub name: String,
    #[serde(default = "default_tun_address")]
    pub address: Ipv4Addr,
    #[serde(default = "default_tun_prefix")]
    pub prefix: u8,
    #[serde(default = "default_tun_destination")]
    pub destination: Ipv4Addr,
    #[serde(default = "default_tun_mtu")]
    pub mtu: u16,
    #[serde(default)]
    pub route: LocalTunRouteConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalTunRouteConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_tun_protect_server_route")]
    pub protect_server_route: bool,
    #[serde(default)]
    pub dns_enabled: bool,
    #[serde(default = "default_tun_dns_servers")]
    pub dns_servers: Vec<IpAddr>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteConfig {
    #[serde(default = "default_remote_listen")]
    pub listen: SocketAddr,
    #[serde(default = "default_handshake_timeout_ms")]
    pub handshake_timeout_ms: u64,
    #[serde(default)]
    pub reject_delay_ms: u64,
    #[serde(default = "default_max_handshake_padding")]
    pub max_handshake_padding: usize,
    #[serde(default = "default_replay_window_secs")]
    pub replay_window_secs: i64,
    #[serde(default = "default_cold_start_delay_ms")]
    pub cold_start_delay_ms: u64,
    #[serde(default = "default_tarpit_max")]
    pub tarpit_max: usize,
    #[serde(default = "default_tarpit_hold_secs")]
    pub tarpit_hold_secs: u64,
    #[serde(default)]
    pub fallback_http: RemoteFallbackHttpConfig,
    #[serde(default)]
    pub egress: EgressConfig,
    #[serde(default)]
    pub users: Vec<RemoteUserConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteUserConfig {
    pub name: String,
    pub psk: String,
    #[serde(default)]
    pub quota: RemoteUserQuotaConfig,
    #[serde(default)]
    pub bandwidth: RemoteUserBandwidthConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteUserQuotaConfig {
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default = "default_user_quota_window_secs")]
    pub window_secs: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RemoteUserBandwidthConfig {
    #[serde(default)]
    pub bytes_per_sec: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteFallbackHttpConfig {
    #[serde(default)]
    pub mode: ProbeDefenseMode,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub upstream: Option<String>,
    #[serde(default = "default_fallback_probe_timeout_ms")]
    pub probe_timeout_ms: u64,
    #[serde(default = "default_fallback_server")]
    pub server: String,
    #[serde(default = "default_fallback_body")]
    pub body: String,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeDefenseMode {
    #[default]
    Silent,
    HttpFallback,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EgressConfig {
    #[serde(default)]
    pub deny_private_ips: bool,
    #[serde(default)]
    pub allow_hosts: Vec<String>,
    #[serde(default)]
    pub block_hosts: Vec<String>,
    #[serde(default)]
    pub allow_ports: Vec<u16>,
    #[serde(default)]
    pub block_ports: Vec<u16>,
    #[serde(default)]
    pub socks5_proxy: Option<String>,
}

impl From<EgressConfig> for EgressPolicy {
    fn from(config: EgressConfig) -> Self {
        Self {
            deny_private_ips: config.deny_private_ips,
            allow_hosts: config.allow_hosts,
            block_hosts: config.block_hosts,
            allow_ports: config.allow_ports,
            block_ports: config.block_ports,
            socks5_proxy: config.socks5_proxy,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub format: LogFormat,
    #[serde(default)]
    pub file: Option<PathBuf>,
    #[serde(default = "default_log_ansi")]
    pub ansi: bool,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Compact,
    Pretty,
    Json,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AdminConfig {
    #[serde(default)]
    pub listen: Option<SocketAddr>,
    #[serde(default)]
    pub token: Option<String>,
}

impl Default for SharedConfig {
    fn default() -> Self {
        Self {
            psk: None,
            clock_skew_secs: default_clock_skew_secs(),
            puzzle_bits: default_puzzle_bits(),
            max_padding: default_max_padding(),
            jitter_ms: 0,
            padding_chance_percent: default_padding_chance_percent(),
            backpressure_threshold_ms: default_backpressure_threshold_ms(),
            backpressure_cooldown_ms: default_backpressure_cooldown_ms(),
            tunnel_buffer: default_tunnel_buffer(),
            idle_timeout_secs: default_idle_timeout_secs(),
            max_streams: default_max_streams(),
            tcp: TcpConfig::default(),
            pacing: PacingConfig::default(),
            obfuscation: ObfuscationConfig::default(),
            stealth: StealthConfig::default(),
        }
    }
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            nodelay: default_tcp_nodelay(),
            keepalive_secs: default_tcp_keepalive_secs(),
            user_timeout_ms: 0,
            send_buffer_bytes: 0,
            recv_buffer_bytes: 0,
            congestion_control: None,
            heartbeat_secs: default_tcp_heartbeat_secs(),
        }
    }
}

impl Default for PacingConfig {
    fn default() -> Self {
        Self {
            enabled: default_pacing_enabled(),
            max_bytes_per_sec: 0,
            burst_bytes: default_pacing_burst_bytes(),
            min_write_bytes: default_pacing_min_write_bytes(),
        }
    }
}

impl Default for ObfuscationConfig {
    fn default() -> Self {
        Self {
            profile: ObfuscationProfile::Balanced,
            randomize_chunks: default_randomize_chunks(),
            min_chunk: default_min_chunk(),
            max_chunk: default_max_chunk(),
        }
    }
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            frame_size: default_stealth_frame_size(),
            tick_ms: default_stealth_tick_ms(),
        }
    }
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            server: None,
            socks5_listen: default_socks5_listen(),
            http_listen: default_http_listen(),
            handshake_padding: default_handshake_padding(),
            auth: None,
            tun: LocalTunConfig::default(),
        }
    }
}

impl Default for LocalTunConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            name: default_tun_name(),
            address: default_tun_address(),
            prefix: default_tun_prefix(),
            destination: default_tun_destination(),
            mtu: default_tun_mtu(),
            route: LocalTunRouteConfig::default(),
        }
    }
}

impl Default for LocalTunRouteConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            protect_server_route: default_tun_protect_server_route(),
            dns_enabled: false,
            dns_servers: default_tun_dns_servers(),
        }
    }
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            listen: default_remote_listen(),
            handshake_timeout_ms: default_handshake_timeout_ms(),
            reject_delay_ms: 0,
            max_handshake_padding: default_max_handshake_padding(),
            replay_window_secs: default_replay_window_secs(),
            cold_start_delay_ms: default_cold_start_delay_ms(),
            tarpit_max: default_tarpit_max(),
            tarpit_hold_secs: default_tarpit_hold_secs(),
            fallback_http: RemoteFallbackHttpConfig::default(),
            egress: EgressConfig::default(),
            users: Vec::new(),
        }
    }
}

impl Default for RemoteUserQuotaConfig {
    fn default() -> Self {
        Self {
            bytes: None,
            window_secs: default_user_quota_window_secs(),
        }
    }
}

impl Default for RemoteFallbackHttpConfig {
    fn default() -> Self {
        Self {
            mode: ProbeDefenseMode::Silent,
            enabled: false,
            upstream: None,
            probe_timeout_ms: default_fallback_probe_timeout_ms(),
            server: default_fallback_server(),
            body: default_fallback_body(),
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: LogFormat::default(),
            file: None,
            ansi: default_log_ansi(),
        }
    }
}

pub fn load_config(input: ConfigInput) -> Result<EspejismoConfig> {
    match (input.path, input.base64) {
        (Some(_), Some(_)) => anyhow::bail!("use either --config or --config-base64, not both"),
        (Some(path), None) => load_config_file(path),
        (None, Some(encoded)) => load_config_base64(&encoded),
        (None, None) => Ok(EspejismoConfig::default()),
    }
}

pub fn load_config_file(path: impl AsRef<Path>) -> Result<EspejismoConfig> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    parse_config(&content).with_context(|| format!("parse {}", path.display()))
}

pub fn load_config_base64(encoded: &str) -> Result<EspejismoConfig> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .context("decode base64 config")?;
    let content = String::from_utf8(bytes).context("config is not UTF-8")?;
    parse_config(&content).context("parse base64 TOML config")
}

pub fn parse_config(content: &str) -> Result<EspejismoConfig> {
    let config: EspejismoConfig = toml::from_str(content)?;
    if let Some(auth) = &config.local.auth {
        auth.validate()?;
    }
    anyhow::ensure!(
        config.local.tun.prefix <= 32,
        "local.tun.prefix must be <= 32"
    );
    anyhow::ensure!(config.local.tun.mtu >= 576, "local.tun.mtu must be >= 576");
    if config.local.tun.route.dns_enabled {
        anyhow::ensure!(
            !config.local.tun.route.dns_servers.is_empty(),
            "local.tun.route.dns_servers must not be empty when dns_enabled is true"
        );
    }
    if let Some(token) = &config.admin.token {
        anyhow::ensure!(!token.is_empty(), "admin.token must not be empty");
    }
    for user in &config.remote.users {
        anyhow::ensure!(
            !user.name.trim().is_empty(),
            "remote.users.name must not be empty"
        );
        anyhow::ensure!(
            !user.psk.trim().is_empty(),
            "remote.users.psk must not be empty"
        );
        if user.quota.bytes.is_some() {
            anyhow::ensure!(
                user.quota.window_secs > 0,
                "remote.users.quota.window_secs must be greater than 0"
            );
        }
        if let Some(bytes_per_sec) = user.bandwidth.bytes_per_sec {
            anyhow::ensure!(
                bytes_per_sec > 0,
                "remote.users.bandwidth.bytes_per_sec must be greater than 0"
            );
        }
    }
    let mut names = std::collections::BTreeSet::new();
    for user in &config.remote.users {
        anyhow::ensure!(
            names.insert(user.name.as_str()),
            "remote.users contains duplicate user name '{}'",
            user.name
        );
    }
    anyhow::ensure!(
        config.shared.obfuscation.min_chunk > 0,
        "shared.obfuscation.min_chunk must be greater than 0"
    );
    anyhow::ensure!(
        config.shared.obfuscation.min_chunk <= config.shared.obfuscation.max_chunk,
        "shared.obfuscation.min_chunk must be <= max_chunk"
    );
    if let Some(algorithm) = &config.shared.tcp.congestion_control {
        anyhow::ensure!(
            !algorithm.trim().is_empty(),
            "shared.tcp.congestion_control must not be empty when provided"
        );
        anyhow::ensure!(
            algorithm
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
            "shared.tcp.congestion_control may contain only letters, numbers, '_' or '-'"
        );
    }
    if config.shared.pacing.enabled {
        anyhow::ensure!(
            config.shared.pacing.burst_bytes > 0,
            "shared.pacing.burst_bytes must be greater than 0 when pacing is enabled"
        );
        anyhow::ensure!(
            config.shared.pacing.min_write_bytes > 0,
            "shared.pacing.min_write_bytes must be greater than 0 when pacing is enabled"
        );
    }
    if config.shared.obfuscation.profile == ObfuscationProfile::Stealth {
        anyhow::ensure!(
            config.shared.stealth.frame_size <= 64 * 1024,
            "shared.stealth.frame_size must be <= 65536"
        );
        anyhow::ensure!(
            config.shared.stealth.frame_size >= 140,
            "shared.stealth.frame_size must be large enough for stealth handshake"
        );
        anyhow::ensure!(
            config.shared.stealth.tick_ms > 0,
            "shared.stealth.tick_ms must be greater than 0"
        );
    }
    if let Some(upstream) = &config.remote.fallback_http.upstream {
        anyhow::ensure!(
            !upstream.trim().is_empty(),
            "remote.fallback_http.upstream must not be empty when provided"
        );
    }
    Ok(config)
}

pub fn example_config() -> String {
    let config = EspejismoConfig {
        shared: SharedConfig {
            psk: Some("change-me-long-random-secret".to_string()),
            ..SharedConfig::default()
        },
        local: LocalConfig {
            server: Some("127.0.0.1:6690".to_string()),
            auth: Some(ProxyAuth {
                username: "local-user".to_string(),
                password: "local-pass".to_string(),
            }),
            ..LocalConfig::default()
        },
        remote: RemoteConfig::default(),
        logging: LogConfig::default(),
        admin: AdminConfig::default(),
    };
    toml::to_string_pretty(&config).expect("example config serializes")
}

pub fn config_to_toml(config: &EspejismoConfig) -> Result<String> {
    toml::to_string_pretty(config).context("serialize TOML config")
}

pub fn encode_config_base64(toml: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(toml.as_bytes())
}

fn default_clock_skew_secs() -> i64 {
    30
}

fn default_puzzle_bits() -> u8 {
    12
}

fn default_max_padding() -> usize {
    64
}

fn default_padding_chance_percent() -> u8 {
    35
}

fn default_backpressure_threshold_ms() -> u64 {
    40
}

fn default_backpressure_cooldown_ms() -> u64 {
    1000
}

fn default_tunnel_buffer() -> usize {
    1024 * 1024
}

fn default_user_quota_window_secs() -> u64 {
    24 * 60 * 60
}

fn default_tcp_nodelay() -> bool {
    true
}

fn default_tcp_keepalive_secs() -> u64 {
    30
}

fn default_tcp_heartbeat_secs() -> u64 {
    30
}

fn default_pacing_enabled() -> bool {
    true
}

fn default_pacing_burst_bytes() -> usize {
    64 * 1024
}

fn default_pacing_min_write_bytes() -> usize {
    1024
}

fn default_socks5_listen() -> Option<SocketAddr> {
    Some("127.0.0.1:6680".parse().expect("valid address"))
}

fn default_http_listen() -> Option<SocketAddr> {
    Some("127.0.0.1:6681".parse().expect("valid address"))
}

fn default_handshake_padding() -> usize {
    256
}

fn default_tun_name() -> String {
    "esptun0".to_string()
}

fn default_tun_address() -> Ipv4Addr {
    Ipv4Addr::new(10, 255, 0, 2)
}

fn default_tun_destination() -> Ipv4Addr {
    Ipv4Addr::new(10, 255, 0, 1)
}

fn default_tun_prefix() -> u8 {
    24
}

fn default_tun_mtu() -> u16 {
    1500
}

fn default_tun_protect_server_route() -> bool {
    true
}

fn default_tun_dns_servers() -> Vec<IpAddr> {
    vec![
        IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
        IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
    ]
}

fn default_remote_listen() -> SocketAddr {
    "0.0.0.0:6690".parse().expect("valid address")
}

fn default_handshake_timeout_ms() -> u64 {
    3000
}

fn default_max_handshake_padding() -> usize {
    1024
}

fn default_replay_window_secs() -> i64 {
    60
}

fn default_cold_start_delay_ms() -> u64 {
    35
}

fn default_tarpit_max() -> usize {
    1024
}

fn default_tarpit_hold_secs() -> u64 {
    300
}

fn default_fallback_probe_timeout_ms() -> u64 {
    250
}

fn default_fallback_server() -> String {
    "nginx".to_string()
}

fn default_fallback_body() -> String {
    "<html><head><title>It works</title></head><body><h1>It works</h1></body></html>".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_ansi() -> bool {
    true
}

fn default_idle_timeout_secs() -> u64 {
    300
}

fn default_max_streams() -> u32 {
    256
}

fn default_randomize_chunks() -> bool {
    true
}

fn default_min_chunk() -> usize {
    1024
}

fn default_max_chunk() -> usize {
    16 * 1024
}

fn default_stealth_frame_size() -> usize {
    DEFAULT_STEALTH_FRAME_SIZE
}

fn default_stealth_tick_ms() -> u64 {
    DEFAULT_STEALTH_TICK_MS
}
