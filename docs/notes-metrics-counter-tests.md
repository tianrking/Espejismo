# Metrics Counter Correctness Tests

## Analysis and change

Reviewed `crates/espejismo-core/src/metrics.rs`, the metrics references in
`docs/research/REFERENCES.md`, and the project boundary in
`docs/POSITIONING.md`. The existing implementation uses relaxed atomics for
process totals and a mutex-protected per-user map. Byte updates take separate
directional values, so a regression could swap directions, lose accumulation,
or accidentally couple the global and per-user totals. `Metrics` is an `Arc`
wrapper, so cloned handles should observe the same counters.

The upstream project list documents architectural and transport references,
not a specific counter-testing pattern; no external counter design is being
copied. This change adds focused unit coverage for independent directional
accumulation, per-user totals across multiple users, and shared state through
a clone. The byte update API now documents the distinction between process
and per-user accounting. Runtime behavior, protocol, and data path are
unchanged; expected performance effect is zero, with no performance gain
claimed.

## Experiment

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core metrics::tests` —
  passed (7 passed, 0 failed). These include the two new counter tests and the
  five existing cardinality and Prometheus label tests.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core` — passed (141 unit
  tests, 1 config example integration test, and 1 doc test; 0 failures).
- This is correctness-only; throughput benchmarking is not applicable.
- The tests cover both byte directions, repeated additions, a zero-byte
  direction, separation between global and user totals, and clone visibility.
  No regressions were observed.
