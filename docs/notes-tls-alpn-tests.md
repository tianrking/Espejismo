# TLS ALPN negotiation boundary tests

## Analysis and scope

The only TLS client in the server is its HTTPS egress proxy. The proxy sends
HTTP/1.1 CONNECT after TLS, and its production rustls config intentionally has
an empty ALPN offer. The core tunnel does not use TLS or ALPN. This matches
`docs/POSITIONING.md`; the transport and negotiation test approach follows the
focused protocol-state testing guidance in `docs/research/REFERENCES.md`
without borrowing protocol camouflage.

Added an in-memory TLS test for the generic rustls negotiation boundaries:
overlapping protocol lists select the server's first common protocol, while
non-empty disjoint lists fail the handshake at both peers. Existing coverage
also checks empty client offers against empty and non-empty server lists, and
asserts the production HTTPS proxy config keeps ALPN empty. These are tests
only; production behavior and expected throughput are unchanged (0% impact).

## Verification

- `cargo test --offline -p espejismo-server tls_alpn_selects_server_preference_and_rejects_mismatch -- --nocapture`: passed; verifies server-preference selection and mismatch errors from both peers over Tokio duplex IO.
- `cargo test --offline -p espejismo-server`: passed; 72 passed, 0 failed, 1 ignored. The ignored test requires loopback bind; all ALPN tests run in memory.
- No benchmark applies because the change adds test coverage only and changes no runtime path.

Conclusion: ALPN selection, mismatch, and empty-list behavior are covered, with no production or throughput regression expected.
