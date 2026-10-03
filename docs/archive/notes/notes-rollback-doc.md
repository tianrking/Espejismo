# Rollback Documentation

## Scope and rationale

The deployment runbook already contained a short rollback checklist, while the
backup and version-compatibility guides described artifact protection and
client/server matching separately. Expanded the rollback section to connect
those details into an operational sequence: identify a known-good pair, stop
services, restore binaries and protected configs, validate before restart,
verify connectivity, and check TUN routes/DNS separately. Added a Docker note
to restore the pinned image and host-side config rather than rebuilding a
mutable tag.

This is documentation only. It preserves the two-binary, operator-managed
deployment model and adds no automatic rollback or service-management behavior.

## Expected improvement

Runtime throughput and resource use are unchanged (0%). The runbook now calls
out the recovery order and the limits of rollback: a restored binary/config
does not reverse credential changes, firewall edits, or operating-system route
state. No quantified recovery-time improvement is claimed because no operator
study was run.

## Validation

- Cross-checked the recovery sequence against `BACKUP.md`,
  `VERSION-COMPATIBILITY.md`, `TUN.md`, the systemd deployment, and the Docker
  guide.
- Reviewed relative links and command examples in the changed section.
- No runtime code changed; Cargo tests and performance benchmarks are not
  applicable to this documentation-only change.
- Outcome: documentation review passed; no code regressions are applicable.
