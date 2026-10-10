# Prometheus Documentation Notes

## Findings and scope

The repository already documents the authenticated admin `GET /metrics`
endpoint, all exported metric families in `docs/deployment/METRICS.md`, and a
Prometheus scrape example with alert rules in
`docs/deployment/MONITORING-ALERTS.md`. The admin implementation accepts
`Authorization: Bearer <token>`, which matches Prometheus' `bearer_token_file`
configuration. The listener is disabled unless `[admin]` is configured.

This task is documentation-only. The project positioning remains unchanged:
metrics are an optional, small admin surface, not a separate exporter or a
public service.

## Change and expected result

- Added a local authenticated `curl` check and an explicit scrape-guide link to
  the metric catalog.
- Clarified in the admin guide that the Prometheus token-file option matches
  the endpoint's bearer authentication.

Expected improvement: operators can move from metric definitions to a working
scrape configuration with less guesswork. There is no runtime, resource-use,
or throughput change; a numeric performance gain is not applicable.

## Verification

- Cross-checked the documented bearer header against `authorized` in
  `crates/espejismo-core/src/admin.rs` and the scrape YAML in
  `docs/deployment/MONITORING-ALERTS.md`.
- Checked the endpoint setup and positioning against
  `docs/deployment/ADMIN.md` and `docs/POSITIONING.md`.
- `git diff --check` — passed.
- Cargo tests and throughput benchmarks are not applicable because only
  documentation changed. No performance increase is claimed.
