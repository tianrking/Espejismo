//! Bounded DNS resolution helpers used during connection setup.

use std::{future::Future, net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use tokio::{net::lookup_host, time::timeout};

/// Bound how long connection setup waits for the platform DNS resolver.
pub const DNS_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn resolve_socket_addrs(authority: &str) -> Result<Vec<SocketAddr>> {
    // Numeric endpoints need no system resolver and should remain available even
    // when the host's DNS service is slow or unavailable.
    if let Ok(addr) = authority.parse::<SocketAddr>() {
        return Ok(vec![addr]);
    }

    let addrs = with_timeout(lookup_host(authority), DNS_RESOLUTION_TIMEOUT, authority).await?;
    collect_addresses(addrs, authority)
}

fn collect_addresses(
    addrs: impl Iterator<Item = SocketAddr>,
    authority: &str,
) -> Result<Vec<SocketAddr>> {
    let addrs: Vec<_> = addrs.collect();
    if addrs.is_empty() {
        anyhow::bail!("DNS lookup returned no addresses: {authority}");
    }
    Ok(addrs)
}

async fn with_timeout<F, T>(future: F, limit: Duration, authority: &str) -> Result<T>
where
    F: Future<Output = std::io::Result<T>>,
{
    timeout(limit, future)
        .await
        .with_context(|| format!("DNS resolution timed out after {limit:?}: {authority}"))?
        .with_context(|| format!("DNS lookup failed: {authority}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dns_timeout_returns_without_waiting_for_resolver() {
        let result = timeout(
            Duration::from_millis(50),
            with_timeout(
                std::future::pending::<std::io::Result<()>>(),
                Duration::from_millis(10),
                "slow.example:443",
            ),
        )
        .await
        .expect("outer timeout should not fire");

        let error = result.unwrap_err().to_string();
        assert!(error.contains("timed out"));
        assert!(error.contains("slow.example:443"));
    }

    #[tokio::test]
    async fn numeric_ipv4_endpoint_bypasses_dns() {
        assert_eq!(
            resolve_socket_addrs("192.0.2.1:443").await.unwrap(),
            vec!["192.0.2.1:443".parse().unwrap()]
        );
    }

    #[tokio::test]
    async fn numeric_ipv6_endpoint_bypasses_dns() {
        assert_eq!(
            resolve_socket_addrs("[2001:db8::1]:8443").await.unwrap(),
            vec!["[2001:db8::1]:8443".parse().unwrap()]
        );
    }

    #[tokio::test]
    async fn malformed_hostname_authority_returns_contextual_error() {
        let error = resolve_socket_addrs("not-an-authority").await.unwrap_err();
        assert!(error.to_string().contains("DNS lookup failed"));
        assert!(error.to_string().contains("not-an-authority"));
    }

    #[test]
    fn empty_resolver_result_is_reported() {
        let error = collect_addresses(std::iter::empty(), "empty.example:443").unwrap_err();
        assert!(error.to_string().contains("returned no addresses"));
        assert!(error.to_string().contains("empty.example:443"));
    }
}
