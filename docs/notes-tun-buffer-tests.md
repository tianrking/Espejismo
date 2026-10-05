# TUN UDP Buffer Boundary Tests

## Findings and approach

`crates/espejismo-client/src/tun.rs` bounds active TUN UDP relay work with a
1,024-permit semaphore. When all permits are held, the receive loop drops the
new datagram. Completed response datagrams use a separate bounded Tokio channel
of the same capacity; relay tasks await queue space, so a stalled netstack
writer applies backpressure instead of growing memory without bound.

The references in `docs/research/REFERENCES.md` point to the bounded buffering
and flow-control practices used by mature transport implementations. This
change makes the existing policy explicit and testable, without changing the
TCP/Yamux tunnel or the project's non-camouflage positioning. Expected
performance change: none; this is a correctness and resource-boundary change.

## Changes

- Factored the TUN UDP semaphore acquisition and response queue construction
  into small private helpers, ensuring tests exercise the same configured
  capacity as production.
- Added a task-capacity boundary test: all 1,024 permits are accepted, the next
  acquisition is rejected, and capacity becomes available after a permit is
  released.
- Added a response-queue boundary test: exactly 1,024 responses fit, the next
  nonblocking send reports `Full`, and sending resumes after one response is
  consumed.
- Updated `docs/deployment/TUN.md` with the in-flight limit and queue behavior.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-client tun::tests`: 3 passed,
  including the task-limit drop boundary, full response-queue boundary, and
  existing backpressure behavior.
- `$HOME/.cargo/bin/cargo test -p espejismo-client`: 34 passed, 0 failed.
- No throughput benchmark was run because this change does not claim a
  performance improvement.
