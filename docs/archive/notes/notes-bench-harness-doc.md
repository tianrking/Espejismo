# Benchmark Harness Documentation

## Findings and plan

`docs/testing/BENCHMARKS.md` already described the direct/proxy setup and
result interpretation, but did not tell a new operator how to build the
optional HTTP source/sink or what the harness does when aggregate-statistics
support is unavailable, a run directory already exists, or a transfer fails.
The script also needs `tail`, `grep`, and `seq` in addition to the listed core
commands. The helper serves generated fixed-length zero-filled downloads and
accepts POST/PUT uploads.

Documented these setup and runtime details against the current script and
`bench-http` CLI/implementation. This keeps the guide runnable and makes
failure/output behavior clear without changing executable behavior or
Espejismo's non-camouflaged TCP/Yamux positioning.

Expected throughput change: 0%. This is documentation-only, so no throughput
benchmark or Rust test rerun is applicable.

## Verification

- Cross-checked build command, URL behavior, helper methods, required shell
  utilities, output collision protection, aggregate-statistics fallback, and
  transfer failure status against source.
- `git diff --check`: passed.
- No benchmark run; no performance change is claimed.
