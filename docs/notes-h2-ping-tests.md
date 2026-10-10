# HTTP/2 PING boundary tests

## Scope and rationale

HTTP/2 PING is a connection-level health check (RFC 9113 §6.7). The existing
test covered `h2::PingPong`'s one-outstanding-user-PING boundary and successful
acknowledgment in both directions. This round adds timeout and malformed-frame
coverage using the existing `h2` decoder over in-memory Tokio duplex streams.
No performance change is intended; throughput improvement is 0%.

The `h2` 0.4 API automatically handles inbound PING frames and exposes no hook
or setting to rate-limit them. An inbound malicious-frequency limit would need
a separate framing/connection layer. This round records that design limitation
instead of claiming protection the code does not provide. No project
positioning or transport behavior is changed.

## Implementation

- `http2_ping_allows_one_outstanding_probe_at_a_time` checks that a second
  in-flight user PING is rejected, then verifies PONGs both ways.
- `http2_ping_wait_can_be_bounded_when_peer_does_not_respond` leaves the peer
  connection undriven, confirms a caller timeout bounds the wait, and verifies
  the timed-out probe remains in flight until connection teardown.
- `http2_ping_rejects_non_eight_byte_payloads` feeds 0-, 7-, and 9-byte PING
  payloads into the raw frame decoder and expects the connection to reject
  each malformed frame.
- The hostile-frequency rate limit is not tested as a project defense because
  the upstream API provides no enforcement point. A flood characterization
  test would not establish protection.
- All tests use `tokio::io::duplex`; no loopback sockets are involved.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core http2_ping_ --offline` —
  passed: all 3 PING tests; no ignored tests involved.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline` — passed: 319
  unit tests passed, 1 existing test ignored, 0 failed; 1 config example
  integration test passed; all 10 HTTP proxy integration tests passed; 1
  doctest passed.
- `$HOME/.cargo/bin/cargo fmt --all -- --check` reports pre-existing formatting
  differences across unrelated files; the changed test block was formatted
  with rustfmt and does not add whole-repository formatting churn.

This is a correctness and boundary-test change, not a performance optimization.
No throughput or latency improvement is claimed.
