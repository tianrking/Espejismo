//! Bounded DNS resolution helpers used during connection setup.

use std::{future::Future, net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use tokio::{net::lookup_host, time::timeout};

/// Bound how long connection setup waits for the platform DNS resolver.
pub const DNS_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn resolve_socket_addrs(authority: &str) -> Result<Vec<SocketAddr>> {
    let addrs = with_timeout(lookup_host(authority), DNS_RESOLUTION_TIMEOUT, authority).await?;
    Ok(addrs.collect())
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
}
