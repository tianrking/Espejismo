# Benchmark Tooling Commit

## Findings and plan

The current `scripts/bench-throughput.sh` already includes the millisecond
timer fallback (`now_ms` uses `date +%s%3N`, then Python, then epoch seconds
converted to milliseconds) and rejects `ESPEJISMO_ROUNDS` below one. Git history
also confirms the timer helper predates this task. The referenced
`notes-archive/round-015/016/017` files are not present in this checkout, so
there is no archive copy available for a direct file comparison.

The remaining requested tooling gaps were unchecked numeric inputs, result
directory reuse that could overwrite prior artifacts, and transfer failures
that were recorded as `ok=false` while the script still exited successfully.
The script now validates its numeric controls before creating artifacts,
refuses an existing run directory, and returns status 1 after writing results
if any transfer failed. These changes improve input/error reporting and protect
benchmark evidence; they are not expected to change throughput measurements.
Expected throughput change: 0%.

## Verification

- `bash -n scripts/bench-throughput.sh`: passed.
- `git diff --check`: passed.
- Throughput benchmark: not run; this is tooling-only work and the task
  explicitly exempts it from the throughput gate.

No throughput improvement is claimed. The available checks show shell syntax
and patch formatting are clean; runtime behavior was not exercised here.
