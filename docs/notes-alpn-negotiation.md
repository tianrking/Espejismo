# ALPN negotiation boundary tests

## Analysis and scope

The server's HTTPS egress proxy uses rustls but sends HTTP/1.1 CONNECT after
the TLS handshake. Its shared `ClientConfig` leaves `alpn_protocols` empty, so
it does not claim HTTP/2 or another application protocol. The core tunnel does
not use TLS or ALPN. This preserves the project's documented refusal to
impersonate application protocols (`docs/POSITIONING.md`). The reference list
is useful here as a reminder to borrow transport implementation techniques,
not another project's camouflage behavior (`docs/research/REFERENCES.md`).

Change: assert the production HTTPS proxy TLS config offers no ALPN, and add a
local TLS integration test where the server advertises `h2` while the client
offers none. Both peers complete the handshake with no negotiated ALPN. This
documents and guards the negotiation boundary without changing runtime
behavior. Expected performance impact: none; this is a test-only change.

## Verification

- `cargo test -p espejismo-server http_chain::tests --offline`: passed, 6
  tests, including the empty-client-offer/server-advertises-`h2` boundary,
  production config ALPN absence, certificate rejection, and handshake timeout.
- `$HOME/.cargo/bin/cargo test -p espejismo-server --offline`: passed, 30
  passed, 0 failed, 1 ignored (loopback bind required).
- `cargo fmt --all -- --check` reports existing formatting differences in
  unrelated files; `rustfmt --edition 2024 crates/espejismo-server/src/http_chain.rs`
  formats the changed source file without editing those unrelated files.
- No benchmark is applicable because runtime code and performance are unchanged.
