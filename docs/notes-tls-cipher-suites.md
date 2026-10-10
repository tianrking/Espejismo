# TLS cipher suite negotiation boundaries

## Findings and approach

The only rustls client configuration in the workspace is the HTTPS egress proxy client in `crates/espejismo-server/src/http_chain.rs`. It previously used `ClientConfig::builder()`, which obtains the process default crypto provider implicitly and therefore inherits that provider's cipher suite list. The workspace enables rustls with the `ring` feature and does not implement TLS camouflage (see `docs/POSITIONING.md`).

Following rustls' provider-based configuration model (also used by the project's rustls integration patterns), configure the HTTPS proxy client with the ring provider explicitly and retain rustls safe default protocol versions (TLS 1.2 and TLS 1.3). This makes the suite boundary reproducible without creating a project-specific or camouflage-oriented suite list. The expected performance impact is neutral: the same provider and safe protocol defaults are used; this is configuration determinism and correctness hardening, not a throughput optimization.

## Changes and validation

- Added in-memory TLS 1.2 coverage that gives client and server two shared suites in opposite orders and checks the server preference when `ignore_client_order` is enabled. A second handshake with disjoint single-suite lists must fail on both peers.
- Added a production HTTPS proxy suite-set assertion covering rejection of TLS_RSA_WITH_AES_128_CBC_SHA and TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA legacy CBC suites. The test checks the configured ring provider rather than a hand-maintained replacement suite list, preserving Rustls' safe defaults.
- `cargo test --offline -p espejismo-server tls_cipher_suites -- --nocapture`: passed (1), covering server preference and no-overlap failure.
- `cargo test --offline -p espejismo-server https_proxy_cipher_suites_exclude_weak_legacy_suites`: passed (1), covering exclusion of weak legacy CBC suites from the HTTPS proxy provider.
- `cargo test --offline -p espejismo-server`: passed (74 passed, 0 failed, 1 ignored). The ignored test requires loopback bind and is unrelated; all in-memory TLS tests ran.
- Performance: no performance claim or benchmark is applicable; negotiated suite behavior is unchanged and no cipher suite is added or removed relative to the workspace's selected ring provider.
