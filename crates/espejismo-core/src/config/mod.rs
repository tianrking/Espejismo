use std::fs;
use std::net::SocketAddr;
use std::path::Path;

use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
pub struct ConfigInput {
    pub path: Option<String>,
    pub base64: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspejismoConfig {
    #[serde(default)]
    pub shared: SharedConfig,
    #[serde(default)]
    pub local: LocalConfig,
    #[serde(default)]
    pub remote: RemoteConfig,
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalConfig {
    pub server: Option<SocketAddr>,
    #[serde(default = "default_socks5_listen")]
    pub socks5_listen: Option<SocketAddr>,
    #[serde(default = "default_http_listen")]
    pub http_listen: Option<SocketAddr>,
    #[serde(default = "default_handshake_padding")]
    pub handshake_padding: usize,
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
}

impl Default for EspejismoConfig {
    fn default() -> Self {
        Self {
            shared: SharedConfig::default(),
            local: LocalConfig::default(),
            remote: RemoteConfig::default(),
        }
    }
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
    toml::from_str(&content).with_context(|| format!("parse {}", path.display()))
}

pub fn load_config_base64(encoded: &str) -> Result<EspejismoConfig> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .context("decode base64 config")?;
    let content = String::from_utf8(bytes).context("config is not UTF-8")?;
    toml::from_str(&content).context("parse base64 TOML config")
}

pub fn example_config() -> String {
    let config = EspejismoConfig {
        shared: SharedConfig {
            psk: Some("change-me-long-random-secret".to_string()),
            ..SharedConfig::default()
        },
        local: LocalConfig {
            server: Some("127.0.0.1:8443".parse().expect("valid address")),
            ..LocalConfig::default()
        },
        remote: RemoteConfig::default(),
    };
    toml::to_string_pretty(&config).expect("example config serializes")
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

fn default_socks5_listen() -> Option<SocketAddr> {
    Some("127.0.0.1:1080".parse().expect("valid address"))
}

fn default_http_listen() -> Option<SocketAddr> {
    Some("127.0.0.1:8080".parse().expect("valid address"))
}

fn default_handshake_padding() -> usize {
    256
}

fn default_remote_listen() -> SocketAddr {
    "0.0.0.0:8443".parse().expect("valid address")
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
