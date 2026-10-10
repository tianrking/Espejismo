# ECH ClientHello boundary tests

## Plan and scope

The existing hello parsers already bound variable envelopes, fixed hello fields, configured padding, and stealth frame sizes. This change adds direct parser tests for the boundary intersections: exact minimum fixed body, missing declared padding, configured padding overflow, stealth frame too small, and exact frame capacity. It does not alter the wire format or handshake behavior. The reference guide's framing guidance from shadowsocks-rust and its call for explicit parser bounds support focused malformed-input testing; Espejismo keeps its native authenticated encrypted handshake and no-camouflage positioning.

## Implementation

Added `client_hello_parsers_enforce_padding_boundaries` in `crates/espejismo-core/src/crypto/mod.rs`. Updated `docs/PROTOCOL.md` to state that declared padding must exist in the payload, extra envelope bytes past the declared hello are ignored, and stealth client padding must fit frame capacity.

## Validation

`$HOME/.cargo/bin/cargo test --offline -p espejismo-core client_hello_parsers_enforce_padding_boundaries` passed (1 targeted test). `$HOME/.cargo/bin/cargo test --offline -p espejismo-core` passed: 220 unit tests, 1 documented-config integration test, 8 HTTP proxy integration tests, and 1 doctest; 1 loopback-bind unit test was ignored as required by the sandbox rule. The new test covers exact minimum input, truncated declared padding, configured padding overflow, frame sizes below/equal/above required capacity. This is a parser correctness change with no runtime behavior or throughput claim.
