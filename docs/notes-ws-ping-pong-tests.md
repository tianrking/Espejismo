# WebSocket PING/PONG boundary tests

## Scope and rationale

The WebSocket underlay in `crates/espejismo-core/src/underlay.rs` answers each
inbound PING with a PONG carrying the same payload and ignores inbound PONG
frames. It does not originate heartbeat PINGs, track PONG deadlines, or impose
an inbound PING rate limit. As with the Xray-core transport abstraction listed
in `docs/research/REFERENCES.md`, WebSocket remains a carrier below Espejismo's
authenticated tunnel; this work does not change that positioning or add
camouflage behavior.

Added in-memory boundary tests for a 32-frame PING burst and for an incomplete
PING whose mask key never arrives. The burst test verifies every payload is
echoed promptly without requiring a peer PONG. The incomplete-frame test
verifies frame parsing remains pending and can be bounded by a caller timeout.
There is no inbound frequency limiter to test as a defense; the current parser
has no rate-limit policy. No runtime behavior or throughput change is intended
(expected throughput change: 0%).

## Verification

- `cargo test -p espejismo-core underlay::tests::websocket --offline` — passed
  all 24 WebSocket tests, including the 32-PING burst, same-payload PONG
  handling, and caller-bounded incomplete PING read.
- `cargo test -p espejismo-core --offline` — passed 341 unit tests, 1 existing
  ignored loopback test, 1 config example integration test, 10 HTTP proxy
  integration tests, and 1 doctest.
- All tests use in-memory duplex streams for this change; no loopback sockets
  are involved.

This is correctness coverage only: expected throughput improvement is 0%; no
runtime code changed.
