# TLS certificate chain boundary tests

## Findings and plan

The HTTPS proxy client uses rustls' WebPKI verifier and bundled Mozilla roots.
Its existing self-signed certificate test covered an untrusted leaf, but not
intermediate-chain building or certificate validity dates. Following rustls'
standard trust-store verification model (also used by Rust proxy clients such
as shadowsocks-rust), I factored the client config constructor so tests can
exercise the production verifier against an isolated test root. The production
configuration still uses only bundled WebPKI roots and retains its TLS/ALPN,
key logging, and early-data settings.

Added test-only root, intermediate, valid leaf, and expired leaf fixtures.
In-memory duplex handshakes cover a valid complete chain, rejection when the
intermediate is omitted, and rejection of an expired leaf. No loopback bind is
used. rustls does not itself validate OCSP response status; existing OCSP tests
cover delivery and rejection propagation to a policy verifier, not proof of a
revoked certificate. Full CRL/OCSP revocation validation remains unsupported
and is not claimed by these chain tests.

Expected outcome: prevent regressions in chain construction and expiry checks;
no runtime or throughput change beyond moving construction into a helper.

## Validation

- `cargo test --offline -p espejismo-server https_proxy_ -- --nocapture`:
  passed; 12 focused HTTPS proxy tests, including all three new chain cases.
- `cargo test --offline -p espejismo-server`: passed; 70 passed, 0 failed,
  1 ignored. The ignored SOCKS relay test requires loopback bind; all new TLS
  checks use in-memory duplex streams.

Conclusion: complete trusted intermediate chains succeed, while missing
intermediates and expired leaves fail the TLS handshake. No performance claim
applies to this correctness-only change. Revocation status remains a documented
gap because the production rustls WebPKI verifier does not validate OCSP/CRLs.
