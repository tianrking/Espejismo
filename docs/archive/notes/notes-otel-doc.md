# OpenTelemetry Documentation Notes

## Findings and scope

The binaries use `tracing` and `tracing-subscriber` for structured local
logging and expose application metrics through the authenticated admin
`GET /metrics` route in Prometheus text format. The workspace dependencies and
source contain no OpenTelemetry SDK, subscriber bridge, OTLP exporter, trace
context propagation, or OTLP metrics exporter. As a result, setting `OTEL_*`
environment variables alone does not enable collector export.

This is documentation-only. The scope preserves the current small operations
model: local formatted logs can use the host's journal or configured file and
Prometheus pulls metrics from the existing admin route. No new service,
dependency, endpoint, or protocol is introduced.

## Change and expected result

- Added an OpenTelemetry/export-path section to `docs/deployment/LOGGING.md`.
- Distinguished the `tracing` instrumentation API from an installed OpenTelemetry
  exporter, explained where current logs go, and documented the separate
  Prometheus scrape path and missing distributed trace propagation.
- Linked the existing monitoring and metric catalog guides for setup/details.

Expected improvement: operators can determine which collector integrations
work today and avoid expecting OTLP traces, logs, or metrics from environment
configuration alone. Runtime behavior, resource use, and throughput are
unchanged; a numerical performance gain is not applicable.

## Verification

- Confirmed dependency and source state by searching `Cargo.toml`,
  `Cargo.lock`, `crates/`, and `docs/` for OpenTelemetry/OTLP names; no SDK or
  exporter implementation is present.
- Cross-checked the current logging output and `/metrics` exposition against
  `crates/espejismo-core/src/logging.rs`, `docs/deployment/ADMIN.md`, and
  `docs/deployment/METRICS.md`.
- `git diff --check` — passed.
- Cargo tests and throughput benchmarks are not applicable because only
  Markdown documentation changed.
