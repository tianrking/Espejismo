# Dashboard documentation notes

## Findings and scope

Reviewed `docs/deployment/GRAFANA.md`, the imported dashboard JSON, and
`docs/deployment/MONITORING-ALERTS.md`. The guide described importing and the
main caveats, but did not explain what individual panels represent or how to
adapt the Job and Instance selectors to a deployment. The dashboard is a
starter view over existing Prometheus metrics; this work changes documentation
only and preserves the project's small-operations model.

## Change and expected result

Expanded the Grafana guide with the selectors' label assumptions, a panel
interpretation guide, and practical PromQL/rate-window customization advice.
It also directs operators to configure alerts separately and keep bearer
credentials out of dashboard variables and queries. This should reduce
operator guesswork when importing and adapting the dashboard; setup time was
not measured. Runtime behavior, resource use, and throughput are unchanged
(expected 0% change).

## Verification

- Parsed `grafana-dashboard.json` as JSON and cross-checked the documented
  selectors, panel expressions, and metric references against the dashboard
  and `METRICS.md`.
- `git diff --check` — passed.
- Cargo tests and throughput benchmarks are not applicable because this is a
  documentation-only change; no runtime performance result is claimed.
