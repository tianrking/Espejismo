# Dial throttle tests

## Findings and approach

Client lane dials are serialized by `connect_lock`; each waiter rechecks the
control after acquiring the lock. Failed connect or handshake attempts increase
that lane's consecutive failure count, which feeds its bounded exponential
backoff with 80–120% jitter. This keeps throttling lane-local and preserves the
project's existing raw TCP / authenticated tunnel model.

The reference list points to retry and loss handling in quic-go and Hysteria2.
This task applies only the transferable bounded-retry testing idea; no transport
or retry policy is changed.

## Changes and expected benefit

- Extracted the saturating lane failure counter update into
  `record_lane_failure`, used by the existing error path.
- Documented why the locked post-acquisition control recheck coalesces a burst
  of demand into one dial.
- Added a deterministic test that walks repeated dial failures through the
  throttle schedule and checks failure-counter saturation at integer limits.
- Expected benefit: catch regressions in the counter-to-delay relationship and
  overflow behavior. Runtime dialing and throughput are unchanged.

## Verification

Ran `cargo test -p espejismo-client`: 43 passed, 0 failed. The new
`repeated_dial_failures_increase_throttle_and_counters_saturate` test covers
zero-delay first attempt, increasing/capped retries, and saturating counters;
existing reconnect storm tests continue to cover jitter bounds and sample
spread. No throughput benchmark applies because this is a correctness-only
change with no runtime policy adjustment.
