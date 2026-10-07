# TLS key logging boundaries

## Scope and approach

The only Rustls client path is the HTTPS egress proxy in `crates/espejismo-server/src/http_chain.rs`. Its shared `ClientConfig` inherits Rustls' default `NoKeyLog`, but that security property was implicit and had no regression assertion. Make the no-key-log choice explicit and test the config's `KeyLog::will_log(label)` boundary. This guards against accidentally exporting TLS traffic secrets through diagnostics while leaving handshake, resumption, cipher selection, and the core tunnel protocol unchanged.

The reference guide's sing-box/Xray transport layering reinforces keeping ordinary TLS behavior isolated to the optional HTTPS proxy adapter. We do not adopt TLS camouflage or add a key logging feature; that would conflict with `docs/POSITIONING.md`'s native, non-impersonating tunnel identity and add sensitive operational output. Expected performance change: neutral; one configuration-time trait object assignment, no per-record work beyond Rustls' disabled logger check.

## Implementation

- Explicitly set Rustls `ClientConfig.key_log` to `NoKeyLog` in the HTTPS proxy TLS config.
- Extend the existing config boundary test to assert `key_log.will_log("CLIENT_TRAFFIC_SECRET_0")` is false alongside ALPN and early-data restrictions.
- No loopback-dependent test was added; the assertion is a synchronous config test.

## Verification

`cargo test -p espejismo-server http_chain::tests --offline` passed: 8 passed, 0 failed, 37 filtered out (plus the bench binary target with 0 tests). Coverage includes shared TLS config behavior, no TLS 1.3 client traffic secrets logged, no early data or ALPN, and existing HTTPS proxy TLS certificate, timeout, OCSP, and session-resumption paths. All tests use in-memory duplex streams or synchronous config checks; no loopback socket was required.

Conclusion: key logging is explicitly disabled on this TLS client path and covered by a config regression test. No throughput claim applies; the change is configuration-only and expected to have neutral performance impact.
