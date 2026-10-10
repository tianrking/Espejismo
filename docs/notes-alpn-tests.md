# ALPN negotiation boundary tests

## Research and plan

`docs/research/REFERENCES.md` points to sing-box and Xray-core for transport
abstraction and protocol boundaries, and explicitly cautions against borrowing
camouflage behavior. This case uses the same boundary discipline: HTTPS proxy
TLS is an authenticated transport for HTTP CONNECT, and the client must not
claim an application protocol it does not speak. `docs/POSITIONING.md` likewise
says Espejismo does not depend on TLS/HTTP protocol camouflage.

The rustls client config previously relied on the library default empty ALPN
list. Keep that policy explicit in the production config, then exercise its
boundary with in-memory TLS handshakes against servers advertising no ALPN,
`h2`, `http/1.1`, and both protocols. Expected outcome: handshake success with
no negotiated ALPN regardless of server advertisement. This correctness change
has no throughput target; expected improvement is prevention of accidental
protocol negotiation after future TLS configuration changes, with no expected
runtime cost beyond explicitly retaining the empty list.

## Implementation and verification

- `crates/espejismo-server/src/http_chain.rs` now explicitly clears ALPN on the
  HTTPS proxy rustls client config.
- Expanded the duplex-stream regression test to cover zero, one, and multiple
  server ALPN advertisements and assert that neither TLS peer negotiates ALPN.
- The test also asserts that the negotiated cipher suite remains in the
  configured HTTPS proxy provider's supported suite set.
- Focused test: `cargo test -p espejismo-server
  https_proxy_tls_never_negotiates_server_advertised_alpn --offline` — passed
  (1 test; four server ALPN cases; no loopback sockets).
- Full package test: `cargo test -p espejismo-server --offline` — passed,
  53 passed, 0 failed, 1 ignored (`relays_tcp_through_two_socks5_hops`,
  requires loopback bind). All non-ignored server tests passed.

## Conclusion

The ALPN policy is explicit and all four advertisement boundaries negotiate no
application protocol. No regression was observed in the server package suite;
there is no performance claim for this correctness-only change.
