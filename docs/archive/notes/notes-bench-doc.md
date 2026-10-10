# Bench Documentation

## Findings and plan

`docs/testing/BENCHMARKS.md` contained historical throughput tables and a
brief methodology, while invocation details were split across the test plan,
the HK2/RK tuning report, and `scripts/bench-throughput.sh`. In particular,
users needed to know the required topology, how to configure direct and proxy
URLs, what the log-risk gate means, which artifacts to inspect, and how to
interpret aggregate and tunnel-cost values.

Documented a runnable multi-round harness example, prerequisites and key
environment controls, generated artifact meanings, failure handling, and
limits on interpreting proxy efficiency and admin counter deltas. This is a
documentation-only change; expected throughput change is 0%, and no benchmark
rerun is appropriate because no executable or benchmark behavior changed.

The explanation follows the project's existing same-window direct/proxy
comparison approach (also used in the HK2/RK tuning report) and preserves the
positioning in `docs/POSITIONING.md`: these measurements characterize this
non-camouflaged encrypted tunnel and test path, not a universal protocol
ranking.

## Verification

- Manually checked the example against `scripts/bench-throughput.sh` defaults,
  required endpoints, artifact names, log-risk behavior, and summary fields.
- `git diff --check`: passed.
- Performance benchmark: not run; this is documentation-only, with no claimed
  throughput gain or regression.
