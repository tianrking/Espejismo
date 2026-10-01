# Version Compatibility Documentation

## Findings and plan

- Workspace packages share a binary release version (`0.1.5` currently),
  while the authenticated handshake has a separate protocol version (`1`).
- The client and server reject unsupported protocol versions; the current
  handshake has no negotiation or fallback. Existing deployment guidance said
  to keep peers compatible but did not define an upgrade order.
- Document the distinction and avoid claiming compatibility based only on
  release-number similarity. Recommend a server-first rollout only when target
  release notes explicitly guarantee compatibility with existing clients;
  otherwise require a coordinated maintenance-window upgrade.
- Add a deployment policy and link it from the update and runbook docs. This
  reduces rollout ambiguity and the risk of leaving an unsupported mixed pair;
  no runtime behavior or measured performance is expected to change.

## Changes

- Added `docs/deployment/VERSION-COMPATIBILITY.md` with protocol matching,
  conditional order, validation, and rollback guidance.
- Linked the policy from `UPDATES.md` and `RUNBOOK.md`.

## Verification

- Manually checked the policy against `docs/PROTOCOL.md`,
  `crates/espejismo-core/src/crypto/mod.rs` protocol-version checks, workspace
  package versions, and existing deployment procedures.
- Documentation-only change; build and tests were not run. No runtime or
  performance change is expected, so no benchmark result applies.
