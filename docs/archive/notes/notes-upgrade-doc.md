# Upgrade Documentation

## Findings and plan

- The deployment runbook already covers binary backup, config validation,
  restart, health checks, and rollback. The compatibility policy distinguishes
  the release number from wire protocol version 1 and does not promise rolling
  upgrades without an explicit release guarantee.
- Config compatibility needs its own callout: TOML has no schema version and
  the parser rejects unknown fields, so renamed or removed options can stop a
  saved config from loading after an upgrade. Operators should edit a copy and
  validate it with the target binaries while preserving secrets and policy.
- The update-check page still described the command as introduced in `v0.0.6`.
  Remove that stale version reference while making clear the command never
  replaces binaries or restarts services. No runtime behavior changes.

## Changes

- Added config compatibility and validation guidance to
  `docs/deployment/VERSION-COMPATIBILITY.md`.
- Clarified the explicit, non-mutating behavior of update checks in
  `docs/deployment/UPDATES.md`.

Expected improvement: operators can distinguish protocol rollout risk from
config parsing risk and validate migrated config before touching live binaries.
No performance or runtime change is expected.

## Verification

- Manually checked statements against `docs/PROTOCOL.md`,
  `docs/deployment/CONFIG.md`, and `crates/espejismo-core/src/config/mod.rs`
  (protocol version 1; unknown TOML fields are rejected).
- Reviewed the linked compatibility/runbook guidance for consistency.
- Documentation-only change; build and tests were not run. No runtime or
  performance change is expected, so no benchmark result applies.
