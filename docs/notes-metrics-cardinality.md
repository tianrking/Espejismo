# Metrics Cardinality Boundary Tests

## Findings and approach

The core collector already caps user labels at 128 values (including
`other`) and failure-reason labels at 32 values. The prior user boundary test
exercised only handshake counts, so it did not protect direction-specific
byte counters or the actual rendered Prometheus series count. The deployment
catalog also described a user-series cap without making clear that the bound
is on user label values and that each value has two directional byte series.

The change expands the existing overflow test to send more than the user
limit through handshake and both directional byte update paths. It checks the
overflow bucket totals and verifies that the rendered handshake and each
directional byte metric family contains exactly 128 series, with the overflow
bytes exposed under the stable `user="other"` label. The documented bound is
therefore explicit at both the label-value and series-family levels. The
implementation and limits remain unchanged; this does not affect tunnel
traffic, protocol behavior, or project positioning.

Expected result: no increase in runtime metric state or label values. The test
guards the existing fixed ceiling of 128 user values and verifies that excess
traffic in both directions remains counted in the overflow series. No data
path performance gain is claimed, so a throughput comparison is not
applicable.

## Verification

- `cargo test -p espejismo-core --offline` — passed: 169 unit tests, 4
  integration tests, 1 documented-config test, and 1 doc test; 0 failures.
- `metrics::tests::user_metric_series_are_bounded_with_overflow_bucket`
  covers overflow aggregation for handshake counts and both byte directions,
  plus exactly 128 rendered series in each tested metric family.
- No throughput benchmark was run: this adds regression coverage and changes
  no runtime or data-path implementation, so there is no performance claim.
