use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::defaults::*;
use crate::egress::EgressPolicy;
use crate::ingress::ProxyAuth;
use crate::protocol::framing::{ChunkPolicy, FrameOptions, ObfuscationProfile};

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

#[derive(Clone, Debug, Default)]
pub struct FrameOptionOverrides {
    pub max_padding: Option<usize>,
    pub jitter_ms: Option<u64>,
    pub padding_chance_percent: Option<u8>,
    pub backpressure_threshold_ms: Option<u64>,
    pub backpressure_cooldown_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedConfig {
    pub psk: Option<String>,
    #[serde(default = "default_clock_skew_secs")]
    pub clock_skew_secs: i64,
    #[serde(default = "default_puzzle_bits")]
    pub puzzle_bits: u8,
    #[serde(default)]
    pub handshake_window: HandshakeWindowConfig,
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
    #[serde(default = "default_max_physical_connections")]
    pub max_physical_connections: u32,
    #[serde(default = "default_key_update_frames")]
    pub key_update_frames: u64,
    #[serde(default)]
    pub tcp: TcpConfig,
    #[serde(default)]
    pub mux: MuxConfig,
    #[serde(default)]
    pub pacing: PacingConfig,
    #[serde(default)]
    pub obfuscation: ObfuscationConfig,
    #[serde(default)]
    pub stealth: StealthConfig,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HandshakeWindowConfig {
    #[serde(default = "default_handshake_window_enabled")]
    pub enabled: bool,
    #[serde(default = "default_handshake_window_step_secs")]
    pub step_secs: u64,
    #[serde(default = "default_handshake_window_previous_windows")]
    pub previous_windows: u8,
    #[serde(default)]
    pub future_windows: u8,
}

impl From<HandshakeWindowConfig> for crate::crypto::HandshakeWindow {
    fn from(value: HandshakeWindowConfig) -> Self {
        Self {
            enabled: value.enabled,
            step_secs: value.step_secs,
            previous_windows: value.previous_windows,
            future_windows: value.future_windows,
        }
    }
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

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MuxMode {
    #[default]
    Yamux,
    Native,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MuxConfig {
    #[serde(default)]
    pub mode: MuxMode,
    #[serde(default = "default_native_mux_initial_window_bytes")]
    pub native_initial_window_bytes: usize,
    #[serde(default = "default_native_mux_stream_buffer_frames")]
    pub native_stream_buffer_frames: usize,
    #[serde(default = "default_native_mux_send_queue_frames")]
    pub native_send_queue_frames: usize,
    #[serde(default = "default_native_mux_idle_timeout_secs")]
    pub native_idle_timeout_secs: u64,
    #[serde(default = "default_native_mux_drain_timeout_secs")]
    pub native_drain_timeout_secs: u64,
}

impl Default for MuxConfig {
    fn default() -> Self {
        Self {
            mode: MuxMode::default(),
            native_initial_window_bytes: default_native_mux_initial_window_bytes(),
            native_stream_buffer_frames: default_native_mux_stream_buffer_frames(),
            native_send_queue_frames: default_native_mux_send_queue_frames(),
            native_idle_timeout_secs: default_native_mux_idle_timeout_secs(),
            native_drain_timeout_secs: default_native_mux_drain_timeout_secs(),
        }
    }
}

impl SharedConfig {
    pub fn frame_options(&self, overrides: &FrameOptionOverrides) -> FrameOptions {
        FrameOptions {
            max_padding: overrides.max_padding.unwrap_or(self.max_padding),
            jitter_ms: overrides.jitter_ms.unwrap_or(self.jitter_ms),
            padding_chance_percent: overrides
                .padding_chance_percent
                .unwrap_or(self.padding_chance_percent),
            backpressure_threshold_ms: overrides
                .backpressure_threshold_ms
                .unwrap_or(self.backpressure_threshold_ms),
            backpressure_cooldown_ms: overrides
                .backpressure_cooldown_ms
                .unwrap_or(self.backpressure_cooldown_ms),
            obfuscation_profile: self.obfuscation.profile,
            chunk_policy: self.obfuscation.chunk_policy,
            randomize_chunks: self.obfuscation.randomize_chunks,
            min_chunk: self.obfuscation.min_chunk,
            max_chunk: self.obfuscation.max_chunk,
            stealth_frame_size: self.stealth.frame_size,
            stealth_frame_size_candidates: self.stealth.frame_size_candidates.clone(),
            stealth_tick_ms: self.stealth.tick_ms,
            pacing_enabled: self.pacing.enabled,
            pacing_max_bytes_per_sec: self.pacing.max_bytes_per_sec,
            pacing_burst_bytes: self.pacing.burst_bytes,
            pacing_min_write_bytes: self.pacing.min_write_bytes,
            heartbeat_secs: self.tcp.heartbeat_secs,
            key_update_frames: self.key_update_frames,
            metrics: None,
        }
    }
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
    #[serde(default)]
    pub chunk_policy: ChunkPolicy,
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
    #[serde(default = "default_stealth_frame_size_candidates")]
    pub frame_size_candidates: Vec<usize>,
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
    #[serde(default)]
    pub tunnel_pool: TunnelPoolConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TunnelPoolConfig {
    #[serde(default = "default_tunnel_pool_min_connections")]
    pub min_connections: usize,
    #[serde(default = "default_tunnel_pool_max_connections")]
    pub max_connections: usize,
    #[serde(default = "default_tunnel_pool_interactive_lanes")]
    pub interactive_lanes: usize,
    #[serde(default = "default_tunnel_pool_bulk_lanes")]
    pub bulk_lanes: usize,
    #[serde(default = "default_tunnel_pool_max_reconnect_attempts")]
    pub max_reconnect_attempts: u32,
    #[serde(default = "default_tunnel_pool_max_connection_age_secs")]
    pub max_connection_age_secs: u64,
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
    pub proxy: Option<String>,
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
            proxy: config.proxy,
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
            handshake_window: HandshakeWindowConfig::default(),
            max_padding: default_max_padding(),
            jitter_ms: 0,
            padding_chance_percent: default_padding_chance_percent(),
            backpressure_threshold_ms: default_backpressure_threshold_ms(),
            backpressure_cooldown_ms: default_backpressure_cooldown_ms(),
            tunnel_buffer: default_tunnel_buffer(),
            idle_timeout_secs: default_idle_timeout_secs(),
            max_streams: default_max_streams(),
            max_physical_connections: default_max_physical_connections(),
            key_update_frames: default_key_update_frames(),
            tcp: TcpConfig::default(),
            mux: MuxConfig::default(),
            pacing: PacingConfig::default(),
            obfuscation: ObfuscationConfig::default(),
            stealth: StealthConfig::default(),
        }
    }
}

impl Default for HandshakeWindowConfig {
    fn default() -> Self {
        Self {
            enabled: default_handshake_window_enabled(),
            step_secs: default_handshake_window_step_secs(),
            previous_windows: default_handshake_window_previous_windows(),
            future_windows: 0,
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
            chunk_policy: ChunkPolicy::Balanced,
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
            frame_size_candidates: default_stealth_frame_size_candidates(),
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
            tunnel_pool: TunnelPoolConfig::default(),
        }
    }
}

impl Default for TunnelPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: default_tunnel_pool_min_connections(),
            max_connections: default_tunnel_pool_max_connections(),
            interactive_lanes: default_tunnel_pool_interactive_lanes(),
            bulk_lanes: default_tunnel_pool_bulk_lanes(),
            max_reconnect_attempts: default_tunnel_pool_max_reconnect_attempts(),
            max_connection_age_secs: default_tunnel_pool_max_connection_age_secs(),
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
