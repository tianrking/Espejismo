# Reconnect backoff unit tests

## Findings and plan

Commit `0a75ab2` moved retry state onto each `TunnelLane` and added a pure
`reconnect_backoff(lane_id, failures)` calculation. Its existing test checked
zero delay, one retry, a loose upper bound, and one pair of distinct lanes, but
did not pin the exponential progression or exact cap and spread behavior.

Add deterministic unit assertions around the existing function only: the
failure-count progression and cap, lane-specific spread, and the fact that
asking for a delay for a second lane does not alter the first lane's result.
This keeps the change at the scheduling-test layer and preserves the project's
protocol and operating model. Expected gain is improved regression detection;
there is no runtime-performance change to estimate.

## Implementation and verification

Replaced the broad backoff assertion with three focused tests. They assert the
500 ms through 16 s exponential sequence, saturation at 16 s, exact spread
values for lane IDs 1 and 2, deterministic repeatability, and independence
between lane inputs.

`$HOME/.cargo/bin/cargo test -p espejismo-client` passed: 28 tests, 0 failed.
This is a correctness-only test change; no throughput benchmark is applicable.
The initial run caught incorrect hand-calculated spread expectations in the new
tests; the assertions were corrected to match the function's documented modulo
calculation, then the full package test run passed. No implementation change
was needed and no regressions were found.
