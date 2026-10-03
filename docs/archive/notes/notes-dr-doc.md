# Disaster recovery documentation

## Findings and scope

Existing `docs/deployment/BACKUP.md` covered config/secrets, deployment files,
basic restore validation, and periodic backup checks. `RUNBOOK.md` covers
upgrade rollback. Neither document gave an incident sequence for clean-host
rebuild, suspected compromise, staged service verification, or explicit TUN
route/DNS recovery. Espejismo has no database or durable application state;
recovery chiefly depends on protected TOML credentials, compatible binaries,
deployment files, and host networking policy.

## Plan and expected result

- Extend the backup guide with containment and assessment, trusted host
  rebuilding, staged validation, credential compromise handling, and closure.
- State that operators set their own recovery time and acceptable data loss
  targets; do not imply software guarantees or database restore semantics.
- Link the guide from README operations docs. Keep advice aligned with the
  existing systemd, Docker, health check, runbook, and TUN guides.
- Documentation only: no runtime or performance change is expected. The
  expected benefit is a repeatable recovery sequence and fewer configuration,
  secret, or route mistakes under incident pressure.

## Validation

- Cross-checked commands, paths, service account, and `--check-config` against
  existing deployment documentation.
- Cross-checked probe and health-check limitations, and linked to TUN crash
  recovery for OS route/DNS cleanup.
- No application tests or throughput benchmark apply to this docs-only change;
  no performance gain is claimed.
