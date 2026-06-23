use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use base64::Engine;

mod defaults;
mod types;

use crate::ingress::ProxyAuth;
use crate::protocol::framing::{ChunkPolicy, ObfuscationProfile};
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
    let validate_stealth_frame_size = |frame_size: usize, field: &str| -> Result<()> {
        anyhow::ensure!(frame_size <= 64 * 1024, "{field} must be <= 65536");
        let min_stealth_frame = 24 + 32 + 84 + 16 + 1;
        anyhow::ensure!(
            frame_size >= min_stealth_frame,
            "{field} must leave room for handshake, AEAD tag, and at least one payload byte"
        );
        Ok(())
    };
    if let Some(auth) = &config.local.auth {
        auth.validate()?;
    }
    anyhow::ensure!(
        config.local.tun.prefix <= 32,
        "local.tun.prefix must be <= 32"
    );
    anyhow::ensure!(config.local.tun.mtu >= 576, "local.tun.mtu must be >= 576");
    anyhow::ensure!(
        config.local.tun.udp_timeout_secs > 0,
        "local.tun.udp_timeout_secs must be greater than 0"
    );
    if config.local.tun.route.dns_enabled {
        anyhow::ensure!(
            !config.local.tun.route.dns_servers.is_empty(),
            "local.tun.route.dns_servers must not be empty when dns_enabled is true"
        );
    }
    if let Some(token) = &config.admin.token {
        anyhow::ensure!(!token.is_empty(), "admin.token must not be empty");
    }
    if let Some(listen) = config.admin.listen {
        anyhow::ensure!(
            listen.ip().is_loopback() || config.admin.token.is_some(),
            "admin.token is required when admin.listen is not loopback"
        );
    }
    anyhow::ensure!(
        config.shared.clock_skew_secs > 0,
        "shared.clock_skew_secs must be greater than 0"
    );
    if config.shared.handshake_window.enabled {
        anyhow::ensure!(
            config.shared.handshake_window.step_secs > 0,
            "shared.handshake_window.step_secs must be greater than 0"
        );
        anyhow::ensure!(
            config.shared.handshake_window.previous_windows <= 4,
            "shared.handshake_window.previous_windows must be <= 4"
        );
        anyhow::ensure!(
            config.shared.handshake_window.future_windows <= 2,
            "shared.handshake_window.future_windows must be <= 2"
        );
        anyhow::ensure!(
            u16::from(config.shared.handshake_window.previous_windows)
                + u16::from(config.shared.handshake_window.future_windows)
                <= 4,
            "shared.handshake_window previous_windows + future_windows must be <= 4"
        );
    }
    anyhow::ensure!(
        config.remote.replay_window_secs > 0,
        "remote.replay_window_secs must be greater than 0"
    );
    anyhow::ensure!(
        config.remote.handshake_timeout_ms > 0,
        "remote.handshake_timeout_ms must be greater than 0"
    );
    anyhow::ensure!(
        config.shared.idle_timeout_secs > 0,
        "shared.idle_timeout_secs must be greater than 0"
    );
    anyhow::ensure!(
        config.shared.max_streams > 0,
        "shared.max_streams must be greater than 0"
    );
    anyhow::ensure!(
        config.shared.max_streams <= 65_535,
        "shared.max_streams must be <= 65535"
    );
    anyhow::ensure!(
        config.shared.max_physical_connections > 0,
        "shared.max_physical_connections must be greater than 0"
    );
    anyhow::ensure!(
        config.shared.max_physical_connections <= 65_535,
        "shared.max_physical_connections must be <= 65535"
    );
    anyhow::ensure!(
        config.shared.key_update_frames > 0,
        "shared.key_update_frames must be greater than 0"
    );
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
        config.local.tunnel_pool.max_connection_age_secs > 0,
        "local.tunnel_pool.max_connection_age_secs must be greater than 0"
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
        config.shared.mux.native_send_queue_frames > 0,
        "shared.mux.native_send_queue_frames must be greater than 0"
    );
    anyhow::ensure!(
        config.shared.mux.native_idle_timeout_secs > 0,
        "shared.mux.native_idle_timeout_secs must be greater than 0"
    );
    anyhow::ensure!(
        config.shared.mux.native_drain_timeout_secs > 0,
        "shared.mux.native_drain_timeout_secs must be greater than 0"
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
        validate_stealth_frame_size(
            config.shared.stealth.frame_size,
            "shared.stealth.frame_size",
        )?;
        anyhow::ensure!(
            config.shared.stealth.tick_ms > 0,
            "shared.stealth.tick_ms must be greater than 0"
        );
    }
    let mut stealth_sizes = std::collections::BTreeSet::new();
    for &frame_size in &config.shared.stealth.frame_size_candidates {
        validate_stealth_frame_size(frame_size, "shared.stealth.frame_size_candidates[]")?;
        anyhow::ensure!(
            stealth_sizes.insert(frame_size),
            "shared.stealth.frame_size_candidates must not contain duplicates"
        );
    }
    if let Some(upstream) = &config.remote.fallback_http.upstream {
        anyhow::ensure!(
            !upstream.trim().is_empty(),
            "remote.fallback_http.upstream must not be empty when provided"
        );
    }
    if config.remote.egress.proxy.is_some() && config.remote.egress.socks5_proxy.is_some() {
        anyhow::bail!("use remote.egress.proxy or remote.egress.socks5_proxy, not both");
    }
    let egress_policy: crate::egress::EgressPolicy = config.remote.egress.clone().into();
    egress_policy.upstream_proxy()?;
    Ok(config)
}

pub fn apply_named_profile(config: &mut EspejismoConfig, name: &str) -> Result<()> {
    let normalized = name.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "fast" => {
            config.shared.max_padding = 0;
            config.shared.padding_chance_percent = 0;
            config.shared.jitter_ms = 0;
            config.shared.obfuscation.profile = ObfuscationProfile::Bulk;
            config.shared.obfuscation.chunk_policy = ChunkPolicy::Bulk;
            config.shared.obfuscation.randomize_chunks = false;
            config.shared.pacing.enabled = true;
            config.shared.pacing.burst_bytes = 128 * 1024;
            config.shared.pacing.min_write_bytes = 4096;
            config.shared.tunnel_buffer = 256 * 1024;
            config.local.tunnel_pool.min_connections = 1;
            config.local.tunnel_pool.max_connections = 4;
            config.local.tunnel_pool.interactive_lanes = 1;
            config.local.tunnel_pool.bulk_lanes = 2;
        }
        "balanced" => {
            config.shared.obfuscation.profile = ObfuscationProfile::Balanced;
            config.shared.obfuscation.chunk_policy = ChunkPolicy::Balanced;
            config.shared.max_padding = defaults::default_max_padding();
            config.shared.padding_chance_percent = defaults::default_padding_chance_percent();
            config.shared.jitter_ms = 0;
            config.shared.pacing.enabled = true;
            config.local.tunnel_pool.min_connections = 1;
            config.local.tunnel_pool.max_connections = 4;
            config.local.tunnel_pool.interactive_lanes = 1;
            config.local.tunnel_pool.bulk_lanes = 2;
        }
        "low-latency" | "latency" => {
            config.shared.max_padding = 16;
            config.shared.padding_chance_percent = 5;
            config.shared.jitter_ms = 0;
            config.shared.obfuscation.profile = ObfuscationProfile::LowLatency;
            config.shared.obfuscation.chunk_policy = ChunkPolicy::LowLatency;
            config.shared.obfuscation.randomize_chunks = true;
            config.shared.tcp.nodelay = true;
            config.shared.pacing.enabled = true;
            config.shared.pacing.burst_bytes = 32 * 1024;
            config.shared.pacing.min_write_bytes = 512;
            config.local.tunnel_pool.min_connections = 1;
            config.local.tunnel_pool.max_connections = 2;
            config.local.tunnel_pool.interactive_lanes = 1;
            config.local.tunnel_pool.bulk_lanes = 1;
        }
        "stealth" => {
            config.shared.obfuscation.profile = ObfuscationProfile::Stealth;
            config.shared.obfuscation.chunk_policy = ChunkPolicy::Stealth;
            config.shared.obfuscation.randomize_chunks = false;
            config.shared.max_padding = 0;
            config.shared.padding_chance_percent = 0;
            config.shared.jitter_ms = 3;
            config.shared.stealth.frame_size = 4096;
            config.shared.stealth.frame_size_candidates = vec![3328, 3584, 4096, 4608];
            config.shared.stealth.tick_ms = 20;
            config.shared.key_update_frames = 8192;
            config.local.handshake_padding = config.local.handshake_padding.max(256);
            config.local.tunnel_pool.min_connections = 1;
            config.local.tunnel_pool.max_connections = 2;
            config.local.tunnel_pool.interactive_lanes = 1;
            config.local.tunnel_pool.bulk_lanes = 1;
        }
        "server-safe" | "safe" => {
            config.remote.egress.deny_private_ips = true;
            config.remote.egress.allow_ports = vec![80, 443];
            config.remote.tarpit_max = config.remote.tarpit_max.min(512);
            config.shared.max_streams = config.shared.max_streams.clamp(1, 128);
            config.shared.max_physical_connections =
                config.shared.max_physical_connections.clamp(1, 512);
            config.shared.pacing.enabled = true;
            config.shared.pacing.burst_bytes = config.shared.pacing.burst_bytes.min(64 * 1024);
        }
        _ => anyhow::bail!(
            "unknown profile '{name}', expected fast, balanced, low-latency, stealth, or server-safe"
        ),
    }
    Ok(())
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
        apply_named_profile, config_to_toml, encode_config_base64, example_config,
        load_config_base64, parse_config,
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
    fn parses_tun_udp_controls() {
        let config = r#"
            [local.tun]
            udp_enabled = true
            udp_timeout_secs = 2
            udp_block_ports = [443, 853]
        "#;

        let config = parse_config(config).unwrap();
        assert!(config.local.tun.udp_enabled);
        assert_eq!(config.local.tun.udp_timeout_secs, 2);
        assert_eq!(config.local.tun.udp_block_ports, vec![443, 853]);
    }

    #[test]
    fn rejects_zero_tun_udp_timeout() {
        let config = r#"
            [local.tun]
            udp_timeout_secs = 0
        "#;

        let err = parse_config(config).unwrap_err().to_string();
        assert!(err.contains("udp_timeout_secs"));
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

    #[test]
    fn rejects_public_admin_listener_without_token() {
        let config = r#"
            [admin]
            listen = "0.0.0.0:9090"
        "#;

        let err = parse_config(config).unwrap_err().to_string();
        assert!(err.contains("admin.token"), "{err}");

        let loopback = r#"
            [admin]
            listen = "127.0.0.1:9090"
        "#;
        parse_config(loopback).unwrap();
    }

    #[test]
    fn validates_remote_egress_proxy_url() {
        let config = r#"
            [remote.egress]
            proxy = "socks5://user:pass@127.0.0.1:1080"
        "#;
        parse_config(config).unwrap();

        let config = r#"
            [remote.egress]
            proxy = "ftp://127.0.0.1:21"
        "#;
        let err = parse_config(config).unwrap_err().to_string();
        assert!(err.contains("remote.egress.proxy must start"), "{err}");

        let config = r#"
            [remote.egress]
            proxy = "http://127.0.0.1:8080"
            socks5_proxy = "127.0.0.1:1080"
        "#;
        let err = parse_config(config).unwrap_err().to_string();
        assert!(err.contains("proxy or remote.egress.socks5_proxy"), "{err}");
    }

    #[test]
    fn rejects_unsafe_shared_limits_and_time_values() {
        for (config, expected) in [
            ("[shared]\nmax_streams = 0\n", "max_streams"),
            (
                "[shared]\nmax_physical_connections = 0\n",
                "max_physical_connections",
            ),
            ("[shared]\nclock_skew_secs = -1\n", "clock_skew_secs"),
            ("[remote]\nreplay_window_secs = 0\n", "replay_window_secs"),
            (
                "[remote]\nhandshake_timeout_ms = 0\n",
                "handshake_timeout_ms",
            ),
            (
                "[local.tunnel_pool]\nmax_connection_age_secs = 0\n",
                "max_connection_age_secs",
            ),
            ("[shared]\nkey_update_frames = 0\n", "key_update_frames"),
        ] {
            let err = parse_config(config).unwrap_err().to_string();
            assert!(err.contains(expected), "{err}");
        }
    }

    #[test]
    fn rejects_too_small_stealth_frame_for_payload() {
        let config = r#"
            [shared.obfuscation]
            profile = "stealth"

            [shared.stealth]
            frame_size = 140
        "#;

        let err = parse_config(config).unwrap_err().to_string();
        assert!(err.contains("stealth.frame_size"), "{err}");
    }

    #[test]
    fn named_profiles_apply_expected_runtime_shape() {
        let mut config = parse_config("").unwrap();
        apply_named_profile(&mut config, "low-latency").unwrap();
        assert_eq!(
            format!("{:?}", config.shared.obfuscation.chunk_policy),
            "LowLatency"
        );
        assert_eq!(config.local.tunnel_pool.max_connections, 2);

        apply_named_profile(&mut config, "stealth").unwrap();
        assert!(config.shared.obfuscation.profile.is_stealth());
        assert_eq!(config.shared.stealth.frame_size, 4096);

        let err = apply_named_profile(&mut config, "unknown").unwrap_err();
        assert!(err.to_string().contains("unknown profile"));
    }

    #[test]
    fn rejects_duplicate_stealth_frame_size_candidates() {
        let config = r#"
            [shared.obfuscation]
            profile = "stealth"

            [shared.stealth]
            frame_size = 4096
            frame_size_candidates = [3328, 4096, 4096]
        "#;
        let err = parse_config(config).unwrap_err().to_string();
        assert!(err.contains("frame_size_candidates"), "{err}");
    }

    #[test]
    fn rejects_too_small_stealth_frame_size_candidate() {
        let config = r#"
            [shared.obfuscation]
            profile = "stealth"

            [shared.stealth]
            frame_size = 4096
            frame_size_candidates = [128]
        "#;
        let err = parse_config(config).unwrap_err().to_string();
        assert!(err.contains("frame_size_candidates"), "{err}");
    }
}
