# Connection limit documentation

## Findings and approach

- `local.tunnel_pool.max_connections` bounds the client-side physical lane
  pool. `min_connections` is the pool's minimum, and configured lane classes
  must fit within the maximum.
- The remote acquires a process-wide physical connection permit immediately
  after accept and before authentication. When capacity is full, it drops the
  accepted socket and records a debug message. The permit is held for the peer
  handler lifetime, including authentication.
- `shared.max_streams` backs two remote semaphores: one process-wide and one
  per physical connection. Process-wide exhaustion ends the requesting peer
  handler immediately. Per-connection exhaustion waits for a permit only up to
  the handler's bounded timeout; failure ends that physical session. The same
  configured value is used for both scopes.
- `remote.tarpit_max` bounds fallback tarpit occupancy separately; the global
  physical connection permit is still acquired before authentication and does
  not grow to accommodate it.

## Change and expected effect

Expanded `docs/deployment/CONFIG.md` to describe the client pool, remote
physical connection cap, process-wide and per-connection stream caps, defaults,
validation range, and full-capacity behavior. This documents operational
expectations without changing Espejismo's no-camouflage, small-operations
positioning (`docs/POSITIONING.md`). Expected effect: operators can predict
which new work is rejected or which session ends when a limit is reached. No
runtime or throughput change is expected for this documentation-only change.

## Verification

- Cross-checked each documented limit and overload path against
  `crates/espejismo-client/src/tunnel.rs`,
  `crates/espejismo-server/src/main.rs`,
  `crates/espejismo-server/src/handler.rs`, and config validation/defaults in
  `crates/espejismo-core/src/config/`.
- `git diff --check` passed. No cargo tests or throughput benchmark apply:
  runtime behavior was not changed.
- Result: the documentation now distinguishes client pool capacity, remote
  physical admission, and the two stream semaphore scopes. No behavior or
  performance regression is expected; numeric performance change is not
  applicable.
