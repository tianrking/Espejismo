# Physical connection limit boundary tests

## Findings and approach

The remote has a global concurrent physical-connection cap implemented with a
Tokio semaphore. The accept loop tries to acquire a permit before spawning a
peer handler; a full semaphore drops the newly accepted socket immediately,
and the permit is released when the handler ends. This is an admission cap,
not a time-based connection rate limiter. No requests-per-second policy or
window is implemented, so this task does not invent one.

The capacity helper clamps zero to one defensively, while configuration
validation restricts `shared.max_physical_connections` to `1..=65535`. Tests
now cover that defensive floor and both configuration endpoints, in addition
to exact saturation, excess rejection, and capacity reuse on permit release.

## Change and expected effect

- Added boundary tests for physical connection capacity and single-permit
  rejection/recovery in `crates/espejismo-server/src/main.rs`.
- Clarified in `docs/deployment/CONFIG.md` that the setting caps concurrent
  connections and that rejected connections do not consume permits.
- No runtime behavior or performance change is expected; the benefit is
  regression coverage around admission boundaries.

## Verification

- `cargo test --offline -p espejismo-server connection_limit_tests`: passed,
  3 connection-limit tests.
- `cargo test --offline -p espejismo-server`: passed, 51 passed, 0 failed,
  1 ignored. The ignored test requires loopback bind and is outside this
  semaphore-only change.
- These are correctness checks; no throughput experiment applies.
