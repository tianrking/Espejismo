# Metrics documentation

## Findings and scope

Reviewed `docs/deployment/METRICS.md` against the collector and exporters in
`crates/espejismo-core/src/metrics.rs` and `crates/espejismo-core/src/admin.rs`,
then checked metric update sites in the client and server handlers. The catalog
already covers all current process, per-user, failure-reason, and local lane
series with their types, labels, bounds, and reset scopes. The project continues
to expose metrics through its optional authenticated admin endpoint; no new
service or runtime component is needed.

## Change and expected result

Clarified that the local and remote byte counters observe the same payload from
opposite ends, so adding both roles double-counts traffic. Operators can use
one role for unique tunnel payload totals, or keep the roles separate to compare
observations during diagnosis. This prevents misleading throughput totals in
Prometheus queries and dashboards. Runtime behavior, resource use, and
performance are unchanged; a numeric performance gain does not apply.

## Verification

- Cross-checked metric names and types with `render_prometheus` and
  `render_runtime_prometheus`, and verified the documented role and scope
  semantics against update sites.
- `git diff --check` — passed.
- Cargo tests and throughput benchmarks are not applicable: this change only
  clarifies documentation and does not alter code or runtime behavior.
