# Grafana Dashboard Documentation Notes

## Findings and scope

The authenticated `/metrics` endpoint already exports process counters and
local tunnel lane gauges. `METRICS.md` documents names and scope, while
`MONITORING-ALERTS.md` explains authenticated Prometheus scraping. No Grafana
dashboard example existed. This is a documentation-only addition and keeps
the existing small-operations model; no exporter, runtime metric, or service
dependency is introduced.

## Change and expected result

- Added `docs/deployment/grafana-dashboard.json`, an importable starter with
  current connection/stream stats, payload rates, handshake and stream rates,
  and local lane mux RTT.
- Added `docs/deployment/GRAFANA.md` with import steps, dashboard assumptions,
  and metric scope caveats; linked it from the monitoring guide.
- Used existing metric names and instance/job selectors. The guide explains
  that bytes are tunnel payload observed independently at each peer and must
  not be summed across both peers as unique traffic.

Expected improvement: operators can import a useful first Grafana view without
translating metric names and labels manually. No runtime, resource-use, or
throughput change is expected (0%); time saved during dashboard setup was not
measured.

## Verification

- Parsed the dashboard JSON and checked panel expressions against the metric
  catalog and exporter names.
- `git diff --check` — passed.
- Cargo tests and throughput benchmarks are not applicable because this is a
  documentation-only change.
