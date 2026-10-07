# DNS-over-TLS boundary tests

## Findings and approach

The requested topic says DNS-over-TLS, but this repository has no DoT client or
DoT configuration. `crates/espejismo-core/src/dns.rs` uses Tokio's platform
resolver; TUN DNS settings only configure the operating system's resolver.
`docs/deployment/DNS.md` documents these boundaries. Adding a DoT protocol
implementation would exceed a boundary-test task and would change the current
resolver model, so this change tests the existing DNS resolver's resource
boundary instead.

Added a regression test that fills the process-local cache to its configured
256-entry cap, inserts one more authority, and verifies the cache remains
bounded and evicts the entry with the earliest expiration. This guards the
memory bound and eviction policy without changing DNS behavior. Expected
benefit is detection of future capacity/eviction regressions; there is no
performance claim or benchmark for this correctness change. The implementation
remains aligned with the documented platform-resolver model and does not
introduce TLS camouflage or a new DNS protocol.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core dns::tests`: 8 passed. Covers
  cache hit and exact TTL expiration, miss, full-capacity insertion and
  earliest-expiry eviction, timeout and contextual resolver failures, empty
  results, and numeric IPv4/IPv6 bypass.
- `$HOME/.cargo/bin/cargo test -p espejismo-core`: 187 passed, 1 failed, and
  1 doctest/integration group was not reached. The failure is the existing
  `tcp::tests::listener_recovers_after_accept_queue_is_drained`, which cannot
  create a loopback listener in this sandbox (`Operation not permitted` at
  `127.0.0.1:0`). The DNS tests all pass; the required full correctness gate is
  not satisfied in this environment.

Conclusion: DNS cache capacity and eviction boundaries are covered. DoT itself
remains unsupported, and the full core suite is blocked by sandbox networking;
no commit message is prepared because the required full-suite gate did not pass.
