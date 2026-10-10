//! Destination policy and outbound proxy selection for tunneled requests.

use std::net::{IpAddr, SocketAddr};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
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

impl std::fmt::Debug for EgressPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EgressPolicy")
            .field("deny_private_ips", &self.deny_private_ips)
            .field("allow_hosts", &self.allow_hosts)
            .field("block_hosts", &self.block_hosts)
            .field("allow_ports", &self.allow_ports)
            .field("block_ports", &self.block_ports)
            .field("proxy", &self.proxy.as_ref().map(|_| "<redacted>"))
            .field("socks5_proxy", &self.socks5_proxy.as_ref().map(|_| "<redacted>"))
            .finish()
    }
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
            bail!("egress target host '{host}' is blocked by remote.egress.block_hosts");
        }
        if !self.allow_hosts.is_empty()
            && !self
                .allow_hosts
                .iter()
                .any(|pattern| host_matches(&normalized_host, pattern))
        {
            bail!("egress target host '{host}' is not in remote.egress.allow_hosts");
        }
        if self.block_ports.contains(&port) {
            bail!("egress target port {port} is blocked by remote.egress.block_ports");
        }
        if !self.allow_ports.is_empty() && !self.allow_ports.contains(&port) {
            bail!("egress target port {port} is not in remote.egress.allow_ports");
        }
        if self.deny_private_ips {
            if let Ok(ip) = normalized_host.parse::<IpAddr>() {
                if is_private_or_special(ip) {
                    bail!("egress target IP '{ip}' is private or special and remote.egress.deny_private_ips is enabled");
                }
            }
        }
        Ok(())
    }

    pub fn validate_resolved_addr(&self, addr: SocketAddr) -> Result<()> {
        if self.deny_private_ips && is_private_or_special(addr.ip()) {
            bail!("resolved egress address {addr} has a private or special IP and remote.egress.deny_private_ips is enabled");
        }
        if self.block_ports.contains(&addr.port()) {
            bail!(
                "resolved egress address {addr} uses port {} blocked by remote.egress.block_ports",
                addr.port()
            );
        }
        if !self.allow_ports.is_empty() && !self.allow_ports.contains(&addr.port()) {
            bail!(
                "resolved egress address {addr} uses port {} not in remote.egress.allow_ports",
                addr.port()
            );
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

#[derive(Clone, PartialEq, Eq)]
pub struct EgressProxy {
    pub kind: EgressProxyKind,
    pub endpoint: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl std::fmt::Debug for EgressProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EgressProxy")
            .field("kind", &self.kind)
            .field("endpoint", &self.endpoint)
            .field("username", &self.username.as_ref().map(|_| "<redacted>"))
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .finish()
    }
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
    let port = port
        .parse::<u16>()
        .context("target authority port must be an integer from 0 to 65535")?;
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
            let [a, b, c, _] = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                // Non-public and protocol-reserved IPv4 ranges are rejected
                // with the same policy as RFC1918 destinations.
                || a == 0
                || a == 100 && (64..=127).contains(&b) // shared address space
                || a == 192 && (b == 0 || b == 88 && c == 99)
                || a == 198 && (b == 18 || b == 19 || b == 51 && c == 100)
                || a == 203 && b == 0 && c == 113
                || a >= 224
        }
        IpAddr::V6(ip) => {
            // Treat IPv4-mapped addresses like their IPv4 destination so they
            // cannot bypass the same egress boundary through IPv6 syntax.
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return is_private_or_special(IpAddr::V4(mapped));
            }
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
                || ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8 // documentation
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
        let error = policy.validate_authority("127.0.0.1:80").unwrap_err();
        assert!(error.to_string().contains("127.0.0.1"));
        assert!(error.to_string().contains("remote.egress.deny_private_ips"));
    }

    #[test]
    fn supports_host_allowlist_and_wildcards() {
        let policy = EgressPolicy {
            allow_hosts: vec!["*.example.com".to_string()],
            ..EgressPolicy::default()
        };
        assert!(policy.validate_authority("api.example.com:443").is_ok());
        let error = policy.validate_authority("example.net:443").unwrap_err();
        assert!(error.to_string().contains("example.net"));
        assert!(error.to_string().contains("remote.egress.allow_hosts"));
    }

    #[test]
    fn wildcard_host_rules_respect_label_boundaries_and_case() {
        let policy = EgressPolicy {
            allow_hosts: vec!["*.Example.com".to_string()],
            ..EgressPolicy::default()
        };
        assert!(policy.validate_authority("EXAMPLE.COM:443").is_ok());
        assert!(policy.validate_authority("api.eu.example.com:443").is_ok());
        assert!(policy.validate_authority("badexample.com:443").is_err());
        assert!(policy.validate_authority("example.com.evil:443").is_err());
    }

    #[test]
    fn block_host_wildcards_cover_apex_and_subdomains_only() {
        let policy = EgressPolicy {
            block_hosts: vec!["*.Example.com".to_string()],
            ..EgressPolicy::default()
        };

        for blocked in ["example.com:443", "API.example.com:443", "a.b.example.com:443"] {
            assert!(policy.validate_authority(blocked).is_err(), "{blocked} should be blocked");
        }
        for allowed in ["badexample.com:443", "example.com.evil:443", "example.net:443"] {
            assert!(policy.validate_authority(allowed).is_ok(), "{allowed} should not match");
        }
    }

    #[test]
    fn exact_block_host_is_case_insensitive_and_does_not_match_suffixes() {
        let policy = EgressPolicy {
            block_hosts: vec!["Metadata.Example.com".to_string()],
            ..EgressPolicy::default()
        };

        assert!(policy.validate_authority("metadata.example.com:443").is_err());
        assert!(policy.validate_authority("sub.metadata.example.com:443").is_ok());
        assert!(policy.validate_authority("metadata.example.com.evil:443").is_ok());
    }

    #[test]
    fn block_rules_override_allows_and_empty_allows_still_block() {
        let policy = EgressPolicy {
            allow_hosts: vec!["*.example.com".into()],
            block_hosts: vec!["admin.example.com".into()],
            allow_ports: vec![0, u16::MAX],
            block_ports: vec![u16::MAX],
            ..EgressPolicy::default()
        };
        assert!(policy.validate_authority("api.example.com:0").is_ok());
        assert!(policy.validate_authority("admin.example.com:0").is_err());
        assert!(policy.validate_authority("api.example.com:65535").is_err());

        let blocks_only = EgressPolicy {
            block_ports: vec![0],
            ..EgressPolicy::default()
        };
        assert!(blocks_only.validate_authority("example.com:1").is_ok());
        assert!(blocks_only.validate_authority("example.com:0").is_err());
    }

    #[test]
    fn deny_private_ips_covers_ipv4_mapped_ipv6_literals_and_resolved_addrs() {
        let policy = EgressPolicy {
            deny_private_ips: true,
            ..EgressPolicy::default()
        };
        assert!(policy.validate_authority("[::ffff:127.0.0.1]:443").is_err());
        assert!(policy
            .validate_resolved_addr("[::ffff:10.0.0.1]:443".parse().unwrap())
            .is_err());
        assert!(policy
            .validate_authority("[::ffff:8.8.8.8]:443")
            .is_ok());
    }

    #[test]
    fn deny_private_ips_rejects_dns_rebinding_address_boundaries() {
        let policy = EgressPolicy {
            deny_private_ips: true,
            ..EgressPolicy::default()
        };

        // Exercise the resolved-address check: a public hostname can resolve
        // differently between validation and dialing (DNS rebinding).
        for ip in [
            "127.0.0.1",       // loopback
            "10.0.0.1",        // private
            "172.31.255.255",  // private upper edge
            "192.168.0.1",     // private
            "169.254.169.254", // link-local metadata
            "100.64.0.1",      // shared address space
            "192.0.2.1",       // documentation/reserved
            "198.18.0.1",      // benchmarking
            "203.0.113.1",     // documentation/reserved
            "224.0.0.1",       // multicast
            "240.0.0.1",       // reserved
            "::",              // unspecified
            "::1",             // loopback
            "fc00::1",         // unique-local
            "fe80::1",         // link-local
            "ff02::1",         // multicast
            "2001:db8::1",     // documentation
        ] {
            let addr = format!("{ip}:443").parse().unwrap_or_else(|_| {
                format!("[{ip}]:443").parse().expect("valid test socket address")
            });
            assert!(policy.validate_resolved_addr(addr).is_err(), "{ip} must be rejected");
        }

        for ip in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
            let addr = format!("{ip}:443").parse().unwrap_or_else(|_| {
                format!("[{ip}]:443").parse().expect("valid test socket address")
            });
            assert!(policy.validate_resolved_addr(addr).is_ok(), "{ip} must remain allowed");
        }
    }

    #[test]
    fn supports_port_rules() {
        let policy = EgressPolicy {
            allow_ports: vec![443],
            ..EgressPolicy::default()
        };
        assert!(policy.validate_authority("example.com:443").is_ok());
        let error = policy.validate_authority("example.com:80").unwrap_err();
        assert!(error.to_string().contains("80"));
        assert!(error.to_string().contains("remote.egress.allow_ports"));
    }

    #[test]
    fn authority_port_errors_name_the_expected_field_and_format() {
        let error = split_authority("example.com:service").unwrap_err();
        assert!(error
            .to_string()
            .contains("target authority port must be an integer from 0 to 65535"));
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

    #[test]
    fn debug_output_redacts_upstream_proxy_credentials() {
        let proxy = EgressProxy::parse("socks://private-user:private-password@127.0.0.1:1080").unwrap();
        let output = format!("{proxy:?}");
        assert!(output.contains("<redacted>"));
        assert!(!output.contains("private-user"));
        assert!(!output.contains("private-password"));

        let policy = EgressPolicy {
            proxy: Some("https://private-user:private-password@example.com:443".into()),
            ..EgressPolicy::default()
        };
        let output = format!("{policy:?}");
        assert!(!output.contains("private-user"));
        assert!(!output.contains("private-password"));
    }
}
