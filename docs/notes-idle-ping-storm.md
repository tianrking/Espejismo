# Idle ping storm

## Findings and change

The repository reference list points to HashiCorp yamux for session-level
keepalive behavior. The vendored `tokio-yamux` session tracked every outstanding
ping, but emitted another ping on every configured interval until an ACK arrived.
Many idle sessions with delayed ACKs could therefore amplify control traffic and
grow per-session ping state during the same delay window.

`crates/tokio-yamux/src/session.rs` now allows only one unacknowledged keepalive
ping per session. A later tick still checks the existing ping against the
30-second timeout; after the ACK removes it, the next tick sends another probe.
This keeps the TCP/yamux transport and protocol unchanged. The expected bound is
one outstanding ping (12 wire bytes per session), independent of the interval
or ACK delay, instead of approximately `ACK delay / keepalive interval` pings.

## Validation

- `cargo test -p tokio-yamux delayed_keepalive_ack_does_not_multiply_idle_pings --lib --offline` passed. The regression calls 1,000 keepalive ticks with a delayed ACK, verifies only one 12-byte frame and one tracked nonce, then verifies a new ping is sent once the outstanding ping is cleared as acknowledged.
- `cargo test -p tokio-yamux keepalive_timeout_and_interval_boundaries_are_sanitized --lib --offline` passed, preserving existing timeout-boundary and interval-sanitization coverage.
- `cargo test -p tokio-yamux --lib --offline` passed all 42 unit tests, including the existing unacknowledged-peer timeout test (30.21 s).
- `cargo test -p tokio-yamux --offline` ran those 42 unit tests successfully, then the `window_update_deadlock` TCP integration test failed before exercising the code because the sandbox denied socket binding (`PermissionDenied`, `crates/tokio-yamux/tests/window_update_deadlock.rs:31`).

This is a control-traffic bound and correctness hardening change, not a claimed
throughput optimization; no throughput percentage is asserted.
