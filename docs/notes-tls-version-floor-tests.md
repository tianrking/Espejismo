# TLS version floor boundary tests

## Findings and approach

The only TLS client configuration in this path is the HTTPS egress proxy in
`crates/espejismo-server/src/http_chain.rs`. It uses rustls' safe protocol
defaults, which currently offer TLS 1.2 and TLS 1.3. The core tunnel itself is
not TLS, consistent with `docs/POSITIONING.md`; this change does not add a TLS
listener or protocol camouflage.

Following the upstream rustls configuration pattern already used here, add an
in-memory handshake regression test against the production HTTPS proxy config.
It checks negotiation at both protocol boundaries: a TLS 1.2-only peer must
work (the floor), and a TLS 1.3-only peer must still work. rustls does not
implement TLS 1.0/1.1, so this test cannot construct a legacy ClientHello and
does not claim a direct downgrade-attack simulation. Its benefit is preventing
an accidental change to the configured minimum or loss of TLS 1.3; expected
performance impact is zero because this is test-only.

## Changes

- Added `https_proxy_tls_version_floor_is_tls12`, which performs handshakes
  over Tokio in-memory duplex streams using the production TLS configuration
  and verifies the negotiated version on both endpoints.
- Reused the existing valid leaf/intermediate/root test chain. The trusted
  root ensures certificate verification stays enabled during these tests.

## Validation

- `cargo test -p espejismo-server https_proxy_tls_version_floor_is_tls12` —
  passed; covered TLS 1.2-only and TLS 1.3-only peers.
- `cargo test -p espejismo-server` — passed: 71 passed, 0 failed, 1 ignored
  (`relays_tcp_through_two_socks5_hops`, requires loopback bind). No TLS test
  required loopback sockets.
- Correctness result: both supported boundary versions negotiate as expected;
  no regressions observed. This is not a throughput change, so no performance
  gain is claimed.
