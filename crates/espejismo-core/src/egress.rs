use std::net::{IpAddr, SocketAddr};

use anyhow::{bail, Result};
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
}
