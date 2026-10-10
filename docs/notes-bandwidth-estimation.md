# Bandwidth estimation boundary tests

## Scope and approach

Espejismo currently has no measured path-bandwidth estimator. The adaptive
throughput helper estimates a bounded bandwidth-delay product from RTT using a
fixed 1 Gbit/s target; the client feeds it smoothed connection/handshake RTT
samples. This keeps the existing TCP/Yamux design and its operator-controlled
settings intact. As the reference list recommends for transport tuning, test
the estimator's limits explicitly rather than treating the assumed rate as a
measured result ([quic-go](https://github.com/quic-go/quic-go),
[hysteria](https://github.com/apernet/hysteria)).

Added regression coverage for the inclusive 100 ms activation boundary (and
values immediately below it), the resulting BDP floors at 100 ms, and the
maximum representable `Duration` saturating at the configured memory bounds.
No runtime algorithm or throughput behavior changed. Expected benefit is
preventing threshold, arithmetic-range, and cap regressions; no throughput
percentage is claimed.

## Verification

- `cargo test -p espejismo-core adaptive_throughput --offline`: 5 passed.
- `cargo test -p espejismo-core --offline`: 201 unit tests passed, 1 ignored
  because it requires loopback bind; 1 documentation example test, 8 HTTP proxy
  integration tests, and 1 doc test passed.
- No network throughput benchmark was run: this change only adds deterministic
  boundary assertions and does not alter the performance implementation. Thus
  there is no performance delta claim or regression inferred from unit tests.
