use std::net::{IpAddr, SocketAddr};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EgressPolicy {
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

impl EgressPolicy {
    pub fn validate_authority(&self, authority: &str) -> Result<()> {
        let (host, port) = split_authority(authority)?;
        let normalized_host = host.to_ascii_lowercase();

        if self
            .block_hosts
            .iter()
            .any(|pattern| host_matches(&normalized_host, pattern))
        {
            bail!("egress host is blocked");
        }
        if !self.allow_hosts.is_empty()
            && !self
                .allow_hosts
                .iter()
                .any(|pattern| host_matches(&normalized_host, pattern))
        {
            bail!("egress host is not allowed");
        }
        if self.block_ports.contains(&port) {
            bail!("egress port is blocked");
        }
        if !self.allow_ports.is_empty() && !self.allow_ports.contains(&port) {
            bail!("egress port is not allowed");
        }
        if self.deny_private_ips {
            if let Ok(ip) = normalized_host.parse::<IpAddr>() {
                if is_private_or_special(ip) {
                    bail!("private or special egress IP is blocked");
                }
            }
        }
        Ok(())
    }

    pub fn validate_resolved_addr(&self, addr: SocketAddr) -> Result<()> {
        if self.deny_private_ips && is_private_or_special(addr.ip()) {
            bail!("resolved private or special egress IP is blocked");
        }
        if self.block_ports.contains(&addr.port()) {
            bail!("resolved egress port is blocked");
        }
        if !self.allow_ports.is_empty() && !self.allow_ports.contains(&addr.port()) {
            bail!("resolved egress port is not allowed");
        }
        Ok(())
    }

    pub fn upstream_proxy(&self) -> Result<Option<EgressProxy>> {
        if let Some(proxy) = &self.proxy {
            return EgressProxy::parse(proxy).map(Some);
        }
        if let Some(proxy) = &self.socks5_proxy {
            return EgressProxy::parse_legacy_socks5(proxy).map(Some);
        }
        Ok(None)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EgressProxyKind {
    Socks4,
    Socks4a,
    Socks5,
    Http,
    Https,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EgressProxy {
    pub kind: EgressProxyKind,
    pub endpoint: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl EgressProxy {
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        if input.is_empty() {
            bail!("remote.egress.proxy must not be empty");
        }
        let (kind, rest) = if let Some(rest) = input.strip_prefix("socks://") {
            (EgressProxyKind::Socks5, rest)
        } else if let Some(rest) = input.strip_prefix("socks5://") {
            (EgressProxyKind::Socks5, rest)
        } else if let Some(rest) = input.strip_prefix("socks4://") {
            (EgressProxyKind::Socks4, rest)
        } else if let Some(rest) = input.strip_prefix("socks4a://") {
            (EgressProxyKind::Socks4a, rest)
        } else if let Some(rest) = input.strip_prefix("http://") {
            (EgressProxyKind::Http, rest)
        } else if let Some(rest) = input.strip_prefix("https://") {
            (EgressProxyKind::Https, rest)
        } else {
            bail!(
                "remote.egress.proxy must start with socks://, socks4://, socks4a://, socks5://, http://, or https://"
            );
        };
        Self::parse_parts(kind, rest)
    }

    pub fn parse_legacy_socks5(input: &str) -> Result<Self> {
        let input = input.trim();
        if input.is_empty() {
            bail!("remote.egress.socks5_proxy must not be empty");
        }
        if input.contains("://") {
            return Self::parse(input);
        }
        Self::parse_parts(EgressProxyKind::Socks5, input)
    }

    fn parse_parts(kind: EgressProxyKind, rest: &str) -> Result<Self> {
        let rest = rest.trim();
        if rest.is_empty() {
            bail!("egress proxy endpoint is empty");
        }
        anyhow::ensure!(
            !rest.contains('/') && !rest.contains('?') && !rest.contains('#'),
            "egress proxy URL must not contain path, query, or fragment"
        );
        let (auth, endpoint) = match rest.rsplit_once('@') {
            Some((auth, endpoint)) => (Some(auth), endpoint),
            None => (None, rest),
        };
        let (username, password) = match auth {
            Some(auth) => {
                let (username, password) = auth.split_once(':').unwrap_or((auth, ""));
                anyhow::ensure!(!username.is_empty(), "egress proxy username is empty");
                (Some(username.to_string()), Some(password.to_string()))
            }
            None => (None, None),
        };
        let endpoint = endpoint.trim();
        split_authority(endpoint).context("egress proxy endpoint must be host:port")?;
        Ok(Self {
            kind,
            endpoint: endpoint.to_string(),
            username,
            password,
        })
    }
}

pub fn split_authority(authority: &str) -> Result<(String, u16)> {
    if let Ok(addr) = authority.parse::<SocketAddr>() {
        return Ok((addr.ip().to_string(), addr.port()));
    }
    let Some((host, port)) = authority.rsplit_once(':') else {
        bail!("target authority must include a port");
    };
    let host = host.trim_matches(['[', ']']);
    let port = port.parse::<u16>()?;
    if host.is_empty() {
        bail!("target host is empty");
    }
    Ok((host.to_string(), port))
}

fn host_matches(host: &str, pattern: &str) -> bool {
    let pattern = pattern.to_ascii_lowercase();
    if let Some(suffix) = pattern.strip_prefix("*.") {
        host == suffix || host.ends_with(&format!(".{suffix}"))
    } else {
        host == pattern
    }
}

fn is_private_or_special(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.octets()[0] == 0
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_private_ip_when_enabled() {
        let policy = EgressPolicy {
            deny_private_ips: true,
            ..EgressPolicy::default()
        };
        assert!(policy.validate_authority("127.0.0.1:80").is_err());
    }

    #[test]
    fn supports_host_allowlist_and_wildcards() {
        let policy = EgressPolicy {
            allow_hosts: vec!["*.example.com".to_string()],
            ..EgressPolicy::default()
        };
        assert!(policy.validate_authority("api.example.com:443").is_ok());
        assert!(policy.validate_authority("example.net:443").is_err());
    }

    #[test]
    fn supports_port_rules() {
        let policy = EgressPolicy {
            allow_ports: vec![443],
            ..EgressPolicy::default()
        };
        assert!(policy.validate_authority("example.com:443").is_ok());
        assert!(policy.validate_authority("example.com:80").is_err());
    }

    #[test]
    fn parses_upstream_proxy_urls() {
        let socks = EgressProxy::parse("socks://user:pass@127.0.0.1:1080").unwrap();
        assert_eq!(socks.kind, EgressProxyKind::Socks5);
        assert_eq!(socks.endpoint, "127.0.0.1:1080");
        assert_eq!(socks.username.as_deref(), Some("user"));
        assert_eq!(socks.password.as_deref(), Some("pass"));

        let socks4a = EgressProxy::parse("socks4a://proxy.example.com:1080").unwrap();
        assert_eq!(socks4a.kind, EgressProxyKind::Socks4a);

        let http = EgressProxy::parse("https://proxy.example.com:8443").unwrap();
        assert_eq!(http.kind, EgressProxyKind::Https);
        assert_eq!(http.endpoint, "proxy.example.com:8443");
        assert!(http.username.is_none());
    }

    #[test]
    fn rejects_unsupported_upstream_proxy_urls() {
        assert!(EgressProxy::parse("socks5://proxy.example.com").is_err());
        assert!(EgressProxy::parse("http://proxy.example.com:8080/path").is_err());
    }
}
