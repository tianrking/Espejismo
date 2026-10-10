# Recommended Alert Rules Documentation

## Findings and scope

`docs/deployment/MONITORING-ALERTS.md` already documents the authenticated
Prometheus scrape, initial handshake and stream failure rules, and their
process-local metric semantics. The configured-target alert handled failed
scrapes (`up == 0`) but not a scrape job whose targets all disappeared, and
the difference between those cases was not stated. A generic Prometheus rule
cannot detect one missing instance while other instances in the job remain;
that requires an expected-target inventory.

This is documentation-only and keeps monitoring within the existing optional
admin endpoint and operator-managed Prometheus setup. It does not add a new
service or change the small-operations model in `docs/POSITIONING.md`.

## Change and expected result

- Added a starter alert for total absence of the `espejismo` scrape job.
- Documented the alert's blind spot for partial target loss and recommended
  using target inventory or service-discovery alerts when per-instance
  completeness matters.
- Clarified that `/healthz` is a liveness probe, not a substitute for scrape
  and tunnel monitoring.

Expected improvement: operators can distinguish scrape failures from missing
target configuration and understand when external inventory is needed. Runtime,
resource use, and throughput are unchanged (0%); no operational-time saving
was measured.

## Verification

- Cross-checked the rules against Prometheus `up` semantics and the endpoint
  scope documented in `METRICS.md`, `HEALTHCHECK.md`, and
  `MONITORING-ALERTS.md`.
- Confirmed this is documentation-only; cargo tests and throughput benchmarks
  are not applicable.
- `git diff --check` — passed.
