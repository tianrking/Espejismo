# On-Call Response Documentation

## Findings and scope

The deployment docs already covered individual operational actions in the
runbook, troubleshooting guide, health-check guide, monitoring guide, and
backup recovery procedure. They did not give an on-call operator a single
ordered path from impact assessment through recovery verification and handoff.

This is a documentation-only change. It keeps the existing optional admin
endpoint, operator-managed alerts, and small deployment model described in
`docs/POSITIONING.md`; it adds no service or runtime behavior.

## Change and expected result

- Added `docs/deployment/ONCALL.md` with an incident sequence: assess scope,
  check process and tunnel health, mitigate, verify traffic recovery, and record
  or hand off the incident.
- Linked the guide from the README deployment index and the deployment runbook.
- Clarified the diagnostic limits of `/healthz` and `--probe-server`, and
  linked existing rollback, TUN, alert, and disaster-recovery procedures.
- Included reminders to protect secrets and coordinate credential changes.

Expected improvement: operators have one discoverable response path and can
verify actual proxy traffic before declaring recovery. Runtime, resource use,
and throughput are unchanged (0%); incident-time savings were not measured.

## Verification

- Cross-checked commands and endpoint limits against `RUNBOOK.md`,
  `TROUBLESHOOTING.md`, `HEALTHCHECK.md`, `ADMIN.md`, `MONITORING-ALERTS.md`,
  `BACKUP.md`, and `TUN.md`.
- Confirmed the additions are documentation-only; cargo tests and throughput
  benchmarks are not applicable.
- `git diff --check` — passed.
- No cargo tests or throughput benchmarks run; runtime code is unchanged.
