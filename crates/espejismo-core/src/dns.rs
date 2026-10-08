//! Bounded DNS resolution helpers used during connection setup.

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    net::SocketAddr,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use tokio::{net::lookup_host, time::timeout};

/// Bound how long connection setup waits for the platform DNS resolver.
pub const DNS_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(10);
// Tokio's system resolver does not expose record TTLs, so cache successful
// hostname results briefly and bound process-wide memory use.
const DNS_CACHE_TTL: Duration = Duration::from_secs(60);
const DNS_CACHE_CAPACITY: usize = 256;
const DNS_MAX_ATTEMPTS: usize = 3;
// Retry briefly inside the existing overall resolution deadline.
const DNS_RETRY_DELAYS: [Duration; DNS_MAX_ATTEMPTS - 1] =
    [Duration::from_millis(100), Duration::from_millis(250)];

fn dns_retry_delay_after_failure(attempt: usize) -> Option<Duration> {
    DNS_RETRY_DELAYS.get(attempt).copied()
}

#[derive(Default)]
struct DnsCache {
    entries: HashMap<String, (Vec<SocketAddr>, Instant)>,
}

impl DnsCache {
    fn get(&mut self, authority: &str, now: Instant) -> Option<Vec<SocketAddr>> {
        match self.entries.get(authority) {
            Some((addrs, expires)) if now < *expires => Some(addrs.clone()),
            Some(_) => {
                self.entries.remove(authority);
                None
            }
            None => None,
        }
    }

    fn insert(&mut self, authority: String, addrs: Vec<SocketAddr>, now: Instant) {
        if self.entries.len() >= DNS_CACHE_CAPACITY && !self.entries.contains_key(&authority) {
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, expires))| *expires)
                .map(|(key, _)| key.clone())
            {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(authority, (addrs, now + DNS_CACHE_TTL));
    }
}

fn cache() -> &'static Mutex<DnsCache> {
    static CACHE: OnceLock<Mutex<DnsCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(DnsCache::default()))
}

pub async fn resolve_socket_addrs(authority: &str) -> Result<Vec<SocketAddr>> {
    // Numeric endpoints need no system resolver and should remain available even
    // when the host's DNS service is slow or unavailable.
    if let Ok(addr) = authority.parse::<SocketAddr>() {
        return Ok(vec![addr]);
    }

    if let Some(addrs) = cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(authority, Instant::now())
    {
        return Ok(addrs);
    }

    let addrs = with_retry(|| lookup_host(authority), DNS_RESOLUTION_TIMEOUT, authority).await?;
    let addrs = collect_addresses(addrs, authority)?;
    cache().lock().unwrap_or_else(|e| e.into_inner()).insert(
        authority.to_owned(),
        addrs.clone(),
        Instant::now(),
    );
    Ok(addrs)
}

fn collect_addresses(
    addrs: impl Iterator<Item = SocketAddr>,
    authority: &str,
) -> Result<Vec<SocketAddr>> {
    // A resolver may surface the same address through multiple records or
    // search paths. Keep the first occurrence so callers do not dial it twice.
    let mut seen = HashSet::new();
    let addrs: Vec<_> = addrs.filter(|addr| seen.insert(*addr)).collect();
    if addrs.is_empty() {
        anyhow::bail!("DNS lookup returned no addresses: {authority}");
    }
    Ok(addrs)
}

async fn with_retry<F, Fut, T>(mut resolve: F, limit: Duration, authority: &str) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::io::Result<T>>,
{
    timeout(limit, async {
        let mut last_error = None;
        for attempt in 0..DNS_MAX_ATTEMPTS {
            match resolve().await {
                Ok(value) => return Ok(value),
                Err(error) => last_error = Some(error),
            }
            if let Some(delay) = dns_retry_delay_after_failure(attempt) {
                tokio::time::sleep(delay).await;
            }
        }
        Err(last_error.expect("DNS retry policy always makes at least one attempt"))
    })
    .await
    .with_context(|| format!("DNS resolution timed out after {limit:?}: {authority}"))?
    .with_context(|| format!("DNS lookup failed after {DNS_MAX_ATTEMPTS} attempts: {authority}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_retry_wait_budget_has_no_delay_after_final_attempt() {
        assert_eq!(dns_retry_delay_after_failure(0), Some(Duration::from_millis(100)));
        assert_eq!(dns_retry_delay_after_failure(1), Some(Duration::from_millis(250)));
        assert_eq!(dns_retry_delay_after_failure(2), None);
        assert_eq!(dns_retry_delay_after_failure(usize::MAX), None);

        let total: Duration = (0..DNS_MAX_ATTEMPTS)
            .filter_map(dns_retry_delay_after_failure)
            .sum();
        assert_eq!(total, Duration::from_millis(350));
        assert!(total < DNS_RESOLUTION_TIMEOUT);
    }

    #[test]
    fn dns_cache_hits_until_ttl_boundary_then_expires() {
        let start = Instant::now();
        let mut cache = DnsCache::default();
        let addr = "192.0.2.10:443".parse().unwrap();
        cache.insert("cache.example:443".into(), vec![addr], start);

        assert_eq!(
            cache.get(
                "cache.example:443",
                start + DNS_CACHE_TTL - Duration::from_nanos(1)
            ),
            Some(vec![addr])
        );
        assert_eq!(cache.get("cache.example:443", start + DNS_CACHE_TTL), None);
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn dns_cache_miss_does_not_create_an_entry() {
        let mut cache = DnsCache::default();
        assert_eq!(cache.get("missing.example:443", Instant::now()), None);
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn dns_cache_stays_bounded_and_evicts_earliest_expiry() {
        let start = Instant::now();
        let mut cache = DnsCache::default();

        for index in 0..DNS_CACHE_CAPACITY {
            let authority = format!("{index}.example:443");
            let addr = format!("192.0.2.{}:443", index % 255 + 1).parse().unwrap();
            cache.insert(
                authority,
                vec![addr],
                start + Duration::from_secs(index as u64),
            );
        }
        assert_eq!(cache.entries.len(), DNS_CACHE_CAPACITY);

        cache.insert(
            "overflow.example:443".into(),
            vec!["192.0.2.200:443".parse().unwrap()],
            start + Duration::from_secs(DNS_CACHE_CAPACITY as u64),
        );

        assert_eq!(cache.entries.len(), DNS_CACHE_CAPACITY);
        assert!(!cache.entries.contains_key("0.example:443"));
        assert!(cache.entries.contains_key("1.example:443"));
        assert!(cache.entries.contains_key("overflow.example:443"));
    }

    #[test]
    fn dns_cache_evicts_entry_nearest_to_expiry_at_capacity() {
        let start = Instant::now();
        let mut cache = DnsCache::default();
        let addr = "192.0.2.10:443".parse().unwrap();

        for index in 0..DNS_CACHE_CAPACITY {
            cache.insert(
                format!("host-{index}.example:443"),
                vec![addr],
                start + Duration::from_millis(index as u64),
            );
        }
        cache.insert("new.example:443".into(), vec![addr], start);

        assert_eq!(cache.entries.len(), DNS_CACHE_CAPACITY);
        assert!(!cache.entries.contains_key("host-0.example:443"));
        assert!(cache.entries.contains_key("host-1.example:443"));
        assert!(cache.entries.contains_key("new.example:443"));
    }

    #[test]
    fn dns_cache_refresh_at_capacity_does_not_evict_another_entry() {
        let start = Instant::now();
        let mut cache = DnsCache::default();
        let addr = "192.0.2.10:443".parse().unwrap();

        for index in 0..DNS_CACHE_CAPACITY {
            cache.insert(format!("host-{index}.example:443"), vec![addr], start);
        }
        cache.insert("host-0.example:443".into(), vec![addr], start);

        assert_eq!(cache.entries.len(), DNS_CACHE_CAPACITY);
        assert!(cache.entries.contains_key("host-0.example:443"));
        assert!(cache.entries.contains_key("host-255.example:443"));
    }

    #[tokio::test]
    async fn dns_timeout_returns_without_waiting_for_resolver() {
        let result = timeout(
            Duration::from_millis(50),
            with_retry(
                || std::future::pending::<std::io::Result<()>>(),
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
    async fn dns_retry_retries_failures_and_stops_after_success() {
        let mut attempts = 0;
        let result = with_retry(
            || {
                attempts += 1;
                async move {
                    if attempts < 3 {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "temporary",
                        ))
                    } else {
                        Ok("resolved")
                    }
                }
            },
            Duration::from_secs(1),
            "retry.example:443",
        )
        .await
        .unwrap();

        assert_eq!(result, "resolved");
        assert_eq!(attempts, 3);
    }

    #[tokio::test]
    async fn dns_retry_reports_last_failure_after_attempt_limit() {
        let mut attempts = 0;
        let error = with_retry(
            || {
                attempts += 1;
                async move {
                    Err::<(), _>(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        format!("failure {attempts}"),
                    ))
                }
            },
            Duration::from_secs(1),
            "failed.example:443",
        )
        .await
        .unwrap_err()
        .to_string();

        assert_eq!(attempts, DNS_MAX_ATTEMPTS);
        assert!(error.contains("after 3 attempts"));
        assert!(error.contains("failed.example:443"));
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

    #[test]
    fn duplicate_resolver_addresses_are_returned_once_in_original_order() {
        let first = "192.0.2.10:443".parse().unwrap();
        let second = "[2001:db8::10]:443".parse().unwrap();
        let addrs = collect_addresses(
            [first, second, first, second, first].into_iter(),
            "multi.example:443",
        )
        .unwrap();

        assert_eq!(addrs, vec![first, second]);
    }
}
