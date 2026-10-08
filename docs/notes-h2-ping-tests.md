# HTTP/2 PING boundary tests

## Scope and rationale

The HTTP/2 underlay already had a byte-stream round trip test, but it did not
exercise connection-level HTTP/2 PING/PONG behavior. `h2` exposes a
`PingPong` handle on each connection. Its API permits one outstanding user
PING; a second send before the matching PONG is rejected. The test now covers
that limit and verifies that probing works in both directions after the first
probe is acknowledged.

This follows HTTP/2's connection-level PING model (RFC 9113, section 6.7) and
uses the `h2` crate API rather than adding another protocol implementation.
That keeps the existing real HTTP/2 underlay behavior and project positioning
unchanged. No performance change is intended; expected throughput improvement
is 0%.

## Implementation

- Added `http2_ping_allows_one_outstanding_probe_at_a_time` in
  `crates/espejismo-core/src/underlay.rs`.
- The test runs client and server over Tokio `duplex`, asserts that a second
  in-flight PING is rejected, waits for its PONG, then verifies the reverse
  direction. One-second timeouts make a missing acknowledgment fail promptly.
- No loopback sockets are used, so the test is runnable inside the sandbox.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core http2_ping_allows_one_outstanding_probe_at_a_time --offline` — passed (1 test; 235 filtered out; no ignored tests involved).
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline` — passed: 235 unit tests passed, 1 existing loopback-bind test ignored, 0 failed; 1 config example integration test passed; 10 HTTP proxy integration tests passed; 1 doctest passed.

This is a correctness and boundary-test change, not a performance optimization;
no benchmark comparison applies. No throughput or latency improvement is
claimed.
