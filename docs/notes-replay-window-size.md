# Replay window size boundary checks

## Findings and approach

`ReplayCache` stores both authenticated first-packet digests and ephemeral
public keys. Its expiry check previously subtracted signed timestamps directly;
extreme values could overflow, and clock rollback behavior was implicit. The
configured TTL remains unchanged and configuration validation already requires
a positive value. As in the existing logic, an entry remains replay-protected
at age equal to the TTL and expires only after that boundary. This matches the
bounded replay-cache approach referenced by HashiCorp Yamux for window
management; the replay identifiers and Espejismo handshake semantics remain
specific to this protocol.

The cache now treats backward time as non-expiring and treats a positive age
that cannot fit in `i64` as expired. No performance benefit is expected; this is
a correctness and robustness fix, so no throughput benchmark is applicable.

## Changes and expected impact

- Replaced unchecked timestamp subtraction with overflow-safe age comparison.
- Added unit coverage for the exact TTL boundary, backward clock movement, and
  `i64::MIN` to `i64::MAX` timestamp span.
- Updated the configuration guide to state expiry boundary semantics.
- Expected throughput change: 0%; normal replay checks retain the same cache
  behavior and require no additional data structure or protocol work.

## Verification

`cargo test -p espejismo-core protocol::replay::tests --offline` passed all 8
replay-cache unit tests, including exact-boundary retention, post-window expiry,
clock rollback, timestamp extremes, distinct identifier types, and atomic
handshake insertion.

`cargo test -p espejismo-core --offline` ran the broader core suite; replay
coverage passed, but unrelated `tcp::tests::listener_recovers_after_accept_queue_is_drained`
failed and the run then stopped responding in another socket-dependent test,
so it was interrupted. This sandbox limitation prevents claiming a clean full
core suite. No replay test failed. `cargo fmt --check` and whole-workspace tests
were not run. No throughput benchmark was run because the change affects only
timestamp boundary correctness and has no expected throughput gain.
