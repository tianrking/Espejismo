# Idle timeout precision

## Findings and approach

The client prunes excess idle tunnel lanes using their last activity timestamp.
That decision previously compared integer Unix seconds, which quantized timeout
expiry to whole seconds and made it sensitive to wall clock adjustments. The
admin snapshot still needs a Unix timestamp, but timeout decisions need an
elapsed-time clock. This matches the explicit idle timeout and drain timeout
approach noted for yamux in `docs/research/REFERENCES.md`; no transport or
product positioning changes are involved.

## Changes and expected benefit

- Track each lane's last activity with `Instant` for pruning, while continuing
  to publish Unix seconds for the existing metric.
- Cover the exact expiration boundary, a timestamp one millisecond before it,
  a future timestamp, and the pool floor / active stream / pending open guards.
- Expected benefit: remove up to one second of timeout-boundary quantization
  and prevent wall clock jumps from changing when excess lanes are reclaimed.
  This is a correctness and resource-lifetime change, not a throughput
  optimization.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-client --offline` passed: all 45
  client unit tests, including the exact timeout boundary and lane-pruning
  guards. The check also compiled the client and core crates.
- No throughput benchmark was run because this is a correctness-only change
  to timeout measurement.
