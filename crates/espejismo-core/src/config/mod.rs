use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use base64::Engine;

mod defaults;
mod types;

use crate::ingress::ProxyAuth;
use crate::protocol::framing::ObfuscationProfile;
pub use types::*;

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
    anyhow::ensure!(
        config.local.tunnel_pool.max_connections > 0,
        "local.tunnel_pool.max_connections must be greater than 0"
    );
    anyhow::ensure!(
        config.local.tunnel_pool.min_connections <= config.local.tunnel_pool.max_connections,
        "local.tunnel_pool.min_connections must be <= max_connections"
    );
    anyhow::ensure!(
        config.local.tunnel_pool.interactive_lanes + config.local.tunnel_pool.bulk_lanes > 0,
        "local.tunnel_pool must configure at least one lane"
    );
    anyhow::ensure!(
        config.local.tunnel_pool.interactive_lanes + config.local.tunnel_pool.bulk_lanes
            <= config.local.tunnel_pool.max_connections,
        "local.tunnel_pool interactive_lanes + bulk_lanes must be <= max_connections"
    );
    anyhow::ensure!(
        config.local.tunnel_pool.max_reconnect_attempts > 0,
        "local.tunnel_pool.max_reconnect_attempts must be greater than 0"
    );
    anyhow::ensure!(
        config.shared.mux.native_initial_window_bytes > 0,
        "shared.mux.native_initial_window_bytes must be greater than 0"
    );
    anyhow::ensure!(
        config.shared.mux.native_stream_buffer_frames > 0,
        "shared.mux.native_stream_buffer_frames must be greater than 0"
    );
    anyhow::ensure!(
        config.shared.mux.native_idle_timeout_secs > 0,
        "shared.mux.native_idle_timeout_secs must be greater than 0"
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

#[cfg(test)]
mod tests {
    use super::{
        config_to_toml, encode_config_base64, example_config, load_config_base64, parse_config,
    };

    #[test]
    fn example_config_roundtrips_through_toml_and_base64() {
        let toml = example_config();
        let config = parse_config(&toml).unwrap();
        assert_eq!(
            config.shared.psk.as_deref(),
            Some("change-me-long-random-secret")
        );
        assert_eq!(config.local.server.as_deref(), Some("127.0.0.1:6690"));

        let encoded = encode_config_base64(&config_to_toml(&config).unwrap());
        let decoded = load_config_base64(&encoded).unwrap();
        assert_eq!(decoded.local.server, config.local.server);
    }

    #[test]
    fn rejects_invalid_tun_prefix_and_mtu() {
        let bad_prefix = r#"
            [local.tun]
            prefix = 33
        "#;
        assert!(parse_config(bad_prefix).is_err());

        let bad_mtu = r#"
            [local.tun]
            mtu = 500
        "#;
        assert!(parse_config(bad_mtu).is_err());
    }

    #[test]
    fn rejects_duplicate_users() {
        let config = r#"
            [[remote.users]]
            name = "alice"
            psk = "change-me-long-random-secret"

            [[remote.users]]
            name = "alice"
            psk = "another-long-random-secret"
        "#;

        let err = parse_config(config).unwrap_err().to_string();
        assert!(err.contains("duplicate user"));
    }

    #[test]
    fn rejects_enabled_dns_route_without_servers() {
        let config = r#"
            [local.tun.route]
            dns_enabled = true
            dns_servers = []
        "#;

        let err = parse_config(config).unwrap_err().to_string();
        assert!(err.contains("dns_servers"));
    }
}
