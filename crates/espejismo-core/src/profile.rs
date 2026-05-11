use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::config::EspejismoConfig;
use crate::ingress::ProxyAuth;

const PROFILE_PREFIX: &str = "espejismo://import/";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientProfile {
    pub name: String,
    pub server: String,
    pub psk: String,
    #[serde(default)]
    pub socks5_listen: Option<SocketAddr>,
    #[serde(default)]
    pub http_listen: Option<SocketAddr>,
    #[serde(default)]
    pub auth: Option<ProxyAuth>,
}

impl ClientProfile {
    pub fn from_config(name: impl Into<String>, config: &EspejismoConfig) -> Result<Self> {
        Ok(Self {
            name: name.into(),
            server: config
                .local
                .server
                .clone()
                .context("local.server is required for a client profile")?,
            psk: config
                .shared
                .psk
                .clone()
                .context("shared.psk is required for a client profile")?,
            socks5_listen: config.local.socks5_listen,
            http_listen: config.local.http_listen,
            auth: config.local.auth.clone(),
        })
    }

    pub fn apply_to_config(self, config: &mut EspejismoConfig) {
        config.shared.psk = Some(self.psk);
        config.local.server = Some(self.server);
        config.local.socks5_listen = self.socks5_listen;
        config.local.http_listen = self.http_listen;
        config.local.auth = self.auth;
    }
}

pub fn encode_profile_url(profile: &ClientProfile) -> Result<String> {
    let json = serde_json::to_vec(profile)?;
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json);
    Ok(format!("{PROFILE_PREFIX}{encoded}"))
}

pub fn decode_profile_url(input: &str) -> Result<ClientProfile> {
    let encoded = input
        .trim()
        .strip_prefix(PROFILE_PREFIX)
        .context("profile URL must start with espejismo://import/")?;
    let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .context("decode profile URL")?;
    serde_json::from_slice(&json).context("parse profile JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_url_roundtrips() {
        let profile = ClientProfile {
            name: "default".to_string(),
            server: "example.com:6690".to_string(),
            psk: "change-me-long-random-secret".to_string(),
            socks5_listen: Some("127.0.0.1:6680".parse().unwrap()),
            http_listen: None,
            auth: None,
        };
        let url = encode_profile_url(&profile).unwrap();
        let decoded = decode_profile_url(&url).unwrap();
        assert_eq!(decoded.name, "default");
        assert_eq!(decoded.server, profile.server);
        assert_eq!(decoded.psk, profile.psk);
    }
}
