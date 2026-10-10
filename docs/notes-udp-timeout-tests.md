# UDP association timeout boundary tests

## Analysis and scope

The SOCKS5 UDP ASSOCIATE handler applies the configured idle duration to each
`recv_from`. When no datagram arrives before that window expires, returning
from the handler drops the UDP socket and its fragment reassembly state. The
previous expression delegated this directly to Tokio's timeout, but had no
client-level regression test for receive success, timeout, or cancellation.

The project references list bounded UDP relay work in shadowsocks-rust as a
useful implementation pattern. This change keeps Espejismo's existing
TCP-carried UDP relay and timeout behavior; it adds testable structure around
the receive boundary without changing the protocol or operational model.

Expected benefit: regression coverage for the association idle deadline and
cleanup path, with no throughput change expected.

## Changes

- Wrapped the association receive timeout in `recv_udp_association_datagram`,
  preserving the existing per-receive inactivity window.
- Added in-memory tests for successful receive before expiry and for timeout
  cancellation of a pending receive future. No loopback socket is needed.
- Documented that expiry exits the handler and drops association-local state.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-client udp_association_ -- --nocapture`:
  passed (2 tests, 0 failed, 54 filtered out). The tests cover success within
  the idle window and expiration plus cancellation/cleanup of a pending receive.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-client`: passed (56
  passed, 0 failed, 0 ignored). This also reran the TUN response timeout
  boundary tests for missing responses, partial headers/payloads, zero timeout,
  and a fresh response window on the next datagram.

No performance claim applies: behavior is unchanged and this is correctness
coverage only.
