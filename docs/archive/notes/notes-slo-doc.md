# SLO Documentation

## Findings and scope

The deployment guides documented process liveness, Prometheus counters, lane
gauges, and starter alerts, but did not describe service-level objectives or
how an operator could measure end-to-end proxy availability. `/healthz` is
explicitly liveness-only. Metrics do not include request-latency histograms or
an end-to-end success ratio; the latest stream-open latency is a gauge and
cannot support percentile calculations.

This change is documentation-only and follows `docs/POSITIONING.md`: it keeps
the one-client/one-server, operator-managed model and adds no service,
protocol, or runtime dependency.

## Change and expected result

- Added `docs/deployment/SLO.md` with example availability, interactive
  latency, stream success, and recovery objectives, along with measurement
  definitions, maintenance accounting, error budget, and response guidance.
- Clearly labeled the values as starting suggestions rather than measured
  product guarantees. Throughput is explicitly left workload-specific.
- Documented what current metrics can and cannot verify so operators use a
  client-side synthetic proxy request for end-to-end SLO measurement.

Expected improvement: operators can define and review measurable goals using
existing deployment primitives, and are less likely to mistake process
liveness or a last-value gauge for end-to-end reliability data. Runtime,
resource use, and throughput change: 0%; no operational-time saving was
measured.

## Verification

- Cross-checked claims against `docs/deployment/HEALTHCHECK.md`,
  `METRICS.md`, `MONITORING-ALERTS.md`, and `RUNBOOK.md`.
- Checked that example error budget arithmetic is consistent: 1% of 30 days
  is 7 hours 12 minutes.
- Documentation-only change; cargo tests and throughput benchmarks are not
  applicable. `git diff --check` — passed.
