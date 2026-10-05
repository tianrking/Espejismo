# Alert documentation update

## Findings and scope

`docs/deployment/MONITORING-ALERTS.md` already contains starter Prometheus
rules for scrape failures, total target absence, handshake failures, and stream
failures. Its response steps omitted the total-target-absence alert, and the
guide did not explicitly caution that quiet tunnels and policy denials are not
automatically signs of service failure. This change is documentation-only and
keeps alert delivery and expected-host inventory under operator control, in
line with `docs/POSITIONING.md`'s small-operations model.

## Change and expected result

- Added response guidance for a missing `espejismo` scrape job, including the
  limitation that the rule cannot detect one missing host when other targets
  still report.
- Clarified that idle tunnels can be healthy and egress denials can be expected
  policy decisions; alert thresholds should reflect a deployment baseline.

Expected benefit: fewer false assumptions during alert triage and clearer
guidance for detecting partial target loss. Runtime behavior, resource use, and
throughput are unchanged (0% expected change); operator response time was not
measured.

## Verification

- Cross-checked the alert names and target-loss limitation against the rules in
  `MONITORING-ALERTS.md`, metric definitions in `METRICS.md`, and process-only
  health semantics in `HEALTHCHECK.md`.
- `git diff --check` — passed.
- Cargo tests and throughput benchmarks are not applicable to this
  documentation-only change; no runtime performance claim is made.
