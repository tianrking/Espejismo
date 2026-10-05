# Retry Backoff Audit

## Findings and plan

- The client tunnel lane reconnect path already used capped exponential backoff
  (500 ms initial delay, 16 s ceiling), but its lane-specific spread was
  deterministic. Restarts and repeated outages could therefore reproduce the
  same retry timing. Replace that fixed spread with fresh bounded ±20% jitter
  per connection attempt; retain the existing cap and immediate initial
  connection.
- Stream open retries are bounded by `local.tunnel_pool.max_reconnect_attempts`.
  A failed connection/handshake returns to the caller instead of spinning
  through that count; subsequent incoming streams drive reconnection through
  the same backoff path.
- Server egress tries each resolved address once. This is address selection,
  not a retry loop. Fallback upstream and doctor reachability probes are also
  one-shot. No other persistent retry loop was found in the client, server, or
  core paths audited.

The approach follows the retry-storm mitigation commonly used by resilient
transport clients: exponential delay with bounded random jitter. The
`docs/research/REFERENCES.md` list points to Hysteria2 and quic-go for transport
retry and loss-handling ideas; this narrowly scoped change does not adopt their
protocols or alter Espejismo's positioning.

## Expected effect

Concurrent reconnects will spread across a ±20% window at each failure count,
reducing synchronized bursts while keeping the worst-case sleep at 16 seconds.
There is no throughput impact during healthy operation; the first connection
still has zero delay.

## Implementation and experiment

The client now chooses a fresh integer multiplier from 80% through 120% for
each reconnect delay. The multiplier is applied to the existing exponential
schedule and the result is clamped to 16 seconds. Added unit coverage checks
zero-delay startup, exponential growth, both jitter bounds, and the hard cap.

Validation results:

- `cargo test -p espejismo-client`: passed, 27 tests.
- `cargo test --workspace`: 180 unit tests passed (client 27, core 119, server
  11, tokio-yamux 23); one integration test could not run successfully because
  the sandbox denied its local socket operation (`PermissionDenied`) in
  `tokio-yamux/tests/window_update_deadlock.rs`. This failure is unrelated to
  reconnect code and requires an environment that permits local sockets.

No throughput benchmark applies: this changes outage retry timing only, and
healthy connection setup retains its zero-delay path. Expected benefit is
reduced retry synchronization, not an increase in steady-state throughput.
