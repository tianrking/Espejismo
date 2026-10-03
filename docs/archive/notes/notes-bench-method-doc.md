# Benchmark Methodology Documentation

## Findings and plan

`docs/testing/BENCHMARKS.md` described how to run the throughput harness and
interpret its output, but did not consolidate the controls needed for a fair
comparison or explain the limits of the harness statistics. The script pairs
direct and proxied cases within rounds and reports median, range, and population
standard deviation; it has no separate warm-up phase, randomized case order,
or confidence interval calculation.

Documented the experiment context to record, paired-case interpretation,
repeat-run guidance, observed spread, and the difference between current
script output and optional formal uncertainty analysis. Clarified that results
describe a particular host/path/workload, with separate analysis for direction
and concurrency. This follows the measurement practices referenced in
`docs/research/REFERENCES.md` without changing the project's TCP/Yamux design
or positioning.

Expected throughput change: 0%. This is documentation-only; no benchmark
rerun is appropriate because no executable or benchmark behavior changed.

## Verification

- Checked the documented statistics and case behavior against
  `scripts/bench-throughput.sh` and `docs/testing/BENCHMARKS.md`.
- No tests or throughput benchmark run; no performance change is claimed.
