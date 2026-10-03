# Health Check Endpoint

## Findings and plan

The admin listener already serves `GET /healthz` with a fixed `ok` body, but
the route currently passes through the admin token gate. This prevents a
load-balancer probe from using the lightweight endpoint without credentials.
The other admin routes expose metrics, runtime state, or configuration actions
and must remain protected.

Make only `GET /healthz` an unauthenticated liveness probe. It will return the
same constant response and disclose no role, version, counters, or tunnel
state. All other routes retain the existing authorization policy. This follows
the small operational model: operators can point a basic TCP/HTTP probe at the
already optional admin listener without introducing another listener or
configuration setting. The listener remains disabled by default and should
remain loopback-bound or firewalled as documented.

Expected impact: load balancers can check process responsiveness without
managing an admin secret; request handling adds only a method/path comparison
and produces no measurable tunnel throughput change.

## Experiment

- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline` — passed: 124 unit
  tests, 1 integration test, and 1 doctest.
- `$HOME/.cargo/bin/cargo test --workspace --offline` — relevant client/core/
  server/yamux unit suites passed (29, 124, 17, and 23 tests respectively),
  but the unrelated `tokio-yamux` integration test
  `one_way_bulk_transfer_exceeding_window` failed because binding a local
  socket is denied by this sandbox (`Operation not permitted`). This is an
  environment limitation; the changed route policy is covered by its unit
  test. No throughput benchmark applies to this correctness-only change.
- `cargo fmt --all -- --check` reports pre-existing formatting differences in
  client, core config, and server files outside this change. The modified Rust
  file was formatted directly.

Conclusion: the targeted crate tests pass with no regression. The full
workspace gate could not complete because its socket-based integration test
requires local networking unavailable in this environment.
