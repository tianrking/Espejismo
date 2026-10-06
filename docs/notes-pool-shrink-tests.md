# Idle connection pool shrink tests

## Findings and approach

The client manager prunes established physical lane controls before a new
stream reservation. Its policy keeps lanes with active streams or pending opens,
waits for 300 seconds of idle time, and stops at the configured
`min_connections`. Existing tests covered individual decision boundaries and
that an empty lane slot stays selectable for its lazy reconnect path, but did
not exercise a sequence of candidates reducing the connected count to the
minimum.

## Change and expected benefit

Added a regression test that models the ordered prune scan across five idle
lanes. It verifies that exactly the three excess controls are eligible, the
connected count reaches two, and another idle candidate cannot lower it
further. This protects the pool's warm floor without changing runtime policy,
connection behavior, or the project's transport positioning. No performance
gain is expected; the benefit is regression coverage for resource cleanup.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-client --offline` passed: 42
  tests, 0 failures. The new `idle_pool_shrink_releases_only_excess_idle_lanes`
  test covers sequential candidate pruning, reaching the configured floor, and
  refusing to shrink below it; the existing tests cover timeout, activity, and
  reconnect-slot behavior.
- This is correctness and resource-lifetime regression coverage, not a throughput
  optimization; no benchmark was run and no throughput gain is claimed.
