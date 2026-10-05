# SLO Documentation

## Findings and scope

`docs/deployment/SLO.md` already provides operator-selected targets and
explains that `/healthz` is liveness-only. The monitoring docs confirm that
current metrics lack end-to-end success ratios and request-latency histograms.
The availability target is a ratio of scheduled probe checks, so its equivalent
time interpretation depends on probe cadence and exclusions. The stream
failure ratio also needs an explicit denominator that includes failed attempts.

This is a documentation-only refinement. It preserves Espejismo's self-managed
small-operations model and makes no runtime, protocol, or positioning change.

## Change and expected result

- Clarified that every scheduled end-to-end probe counts once and that timeouts
  or unsuccessful responses are failures.
- Defined stream establishment failure rate as failed attempts divided by all
  attempted sampled requests, including failures.
- Made the availability error-budget conversion conditional on one-minute
  sampling with no exclusions: 1% is 432 failed checks, or 7 hours 12 minutes
  of equivalent time in a 30-day window.

Expected improvement: operators can calculate the example ratios with a clear
denominator and avoid treating equivalent failed-check time as a prediction of
actual downtime. No runtime or performance improvement is claimed (0% expected
throughput/latency change).

## Verification

- Cross-checked endpoint and metrics limitations against
  `docs/deployment/HEALTHCHECK.md`, `METRICS.md`, and
  `MONITORING-ALERTS.md`; checked rollback guidance against `RUNBOOK.md`.
- Rechecked the arithmetic: 30 days × 1,440 checks/day × 1% = 432 failed
  checks; 432 minutes = 7 hours 12 minutes.
- Confirmed `docs/deployment/SLO.md` is already linked from
  `docs/deployment/INDEX.md`.
- Documentation-only change; build, tests, and throughput benchmarks do not
  apply. Markdown whitespace/link validation is recorded with the final diff.
