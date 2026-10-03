# Monitoring Alert Guide

## Findings and scope

The admin endpoint already exposes authenticated Prometheus metrics, and
`docs/deployment/METRICS.md` catalogs their types and labels. Operators lacked
a concrete scrape example and alerting guidance. `/healthz` is unauthenticated
but reports liveness only; Prometheus can send the supported Bearer token
through its `bearer_token_file` setting.

This is documentation-only. The examples use existing scrape and counter
semantics and preserve the single-config, small-operations model described in
`docs/POSITIONING.md`.

## Change and expected result

- Added `docs/deployment/MONITORING-ALERTS.md` with loopback admin setup,
  Prometheus scrape configuration, sample scrape/handshake/stream alerts, and
  response pointers.
- Linked the guide from `docs/deployment/ADMIN.md`.
- Explained that thresholds are starting points, metrics are process-local,
  counter windows handle restarts, and byte counters are payload totals.

Expected improvement: operators can configure a basic authenticated scrape
and adapt useful initial alerts without guessing endpoint or metric behavior.
No runtime, resource-use, or throughput change is expected (0%); a numeric
operational time saving has not been measured.

## Verification

- Cross-checked endpoint authentication, metric names, and metric semantics
  against `ADMIN.md`, `METRICS.md`, and the existing admin implementation.
- `git diff --check` — passed.
- Cargo tests and throughput benchmarks are not applicable because no code or
  runtime behavior changed.
