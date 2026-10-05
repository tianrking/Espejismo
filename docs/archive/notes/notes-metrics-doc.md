# Metrics Documentation

## Findings and scope

The admin `/metrics` endpoint joins two exporters: process-wide and per-user
metrics from `crates/espejismo-core/src/metrics.rs`, plus local tunnel lane
runtime metrics from `crates/espejismo-core/src/admin.rs`. Existing admin docs
listed broad categories but did not define every exported series, label, type,
scope, or unit. In particular, scrape-time lane gauges and cumulative process
counters needed to be distinguished.

The task is documentation-only. The local references and positioning were
reviewed; no protocol or runtime design changes are needed, and the resulting
operations guide preserves the project's small operations model.

## Change and expected result

- Added `docs/deployment/METRICS.md` with all process, remote per-user,
  remote failure-reason, and local lane metric names and meanings.
- Documented metric types, units, role scope, labels, bounded user/reason
  series, restart behavior, payload byte-counter semantics, and PromQL examples.
- Linked the full catalog from the admin endpoint guide.

Expected improvement: operators can map every exported series to its meaning
and labels when configuring Prometheus queries and dashboards. Runtime,
resource use, and throughput are unchanged; a numeric performance gain is not
applicable to this documentation-only change.

## Verification

- Cross-checked the catalog against `render_prometheus` in `metrics.rs` and
  `render_runtime_prometheus` in `admin.rs`; the document covers every emitted
  series and conditional lane series.
- `git diff --check` — passed.
- Cargo tests and throughput benchmarks are not applicable because no code or
  runtime behavior changed. No performance increase is claimed.
