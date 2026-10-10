# QUIC Version Tests

## Scope and plan

The repository does not implement QUIC transport, and the positioning and
reference guide explicitly keep the core tunnel on its own authenticated
protocol over TCP/yamux. The relevant version boundary is the existing `u16`
wire protocol version in the authenticated handshake. Keep its current exact
match policy and add an explicit predicate with boundary coverage: current
version `1` is supported; zero, the next value, and `u16::MAX` are rejected on
the same predicate used for both client and server checks. No wire format or
compatibility behavior changes. Expected benefit is regression protection,
with no performance claim or throughput change.

The narrow policy follows the repository's quic-go reference lesson to make
protocol state handling explicit, without adopting QUIC or changing project
positioning. See `docs/research/REFERENCES.md` and `docs/POSITIONING.md`.

## Changes and verification

- Added `supports_protocol_version` and routed both handshake directions
  through it.
- Documented exact-match semantics and unsupported numeric boundaries in
  `docs/PROTOCOL.md`.
- Added a unit test covering `0`, current version `1`, `2`, and `u16::MAX`.
- Verification: `cargo test -p espejismo-core --offline` passed (222 passed,
  1 ignored loopback test). The new unit test covers the supported value and
  below/above/maximum `u16` boundaries. No loopback test was added. Result:
  no correctness regression; performance is unchanged because this is a
  constant-time version predicate replacing equivalent equality checks.
