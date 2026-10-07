# TLS cipher suite negotiation boundaries

## Findings and approach

The only rustls client configuration in the workspace is the HTTPS egress proxy client in `crates/espejismo-server/src/http_chain.rs`. It previously used `ClientConfig::builder()`, which obtains the process default crypto provider implicitly and therefore inherits that provider's cipher suite list. The workspace enables rustls with the `ring` feature and does not implement TLS camouflage (see `docs/POSITIONING.md`).

Following rustls' provider-based configuration model (also used by the project's rustls integration patterns), configure the HTTPS proxy client with the ring provider explicitly and retain rustls safe default protocol versions (TLS 1.2 and TLS 1.3). This makes the suite boundary reproducible without creating a project-specific or camouflage-oriented suite list. The expected performance impact is neutral: the same provider and safe protocol defaults are used; this is configuration determinism and correctness hardening, not a throughput optimization.

## Changes and validation

- Made the ring provider and safe TLS 1.2/1.3 defaults explicit for HTTPS proxy TLS.
- Extended the local ALPN negotiation test to assert that a successfully negotiated suite belongs to the production HTTPS proxy configuration's offered suite list. This exercises negotiation at the configured boundary while preserving the no-ALPN behavior.
- Test evidence: `cargo test -p espejismo-server http_chain::tests --offline` passed all 6 targeted HTTPS proxy tests; `cargo test --workspace --offline` passed the client (45), core (183 unit + 1 config integration + 5 HTTP proxy integration), server (36 passed, 1 ignored), and tokio-yamux unit (45) tests, but the tokio-yamux `window_update_deadlock` integration test could not bind its local socket in this sandbox (`PermissionDenied`, line 31). This failure is outside the TLS change; full workspace success is therefore not claimed.
- Performance: no performance claim or benchmark is applicable; negotiated suite behavior is unchanged and no cipher suite is added or removed relative to the workspace's selected ring provider.
