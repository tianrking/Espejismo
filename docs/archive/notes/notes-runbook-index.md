# Runbook index notes

## Scope and rationale

- Added `docs/deployment/INDEX.md` as a task-oriented directory for the
  deployment and operations documentation already in the repository.
- Grouped links around common operator tasks: getting started, installation,
  security/network configuration, recovery, monitoring, and project references.
- Linked the index from the README Operations Docs section while retaining its
  compact list of frequently used guides.
- Kept the existing single-config, two-binary model and accurately describe
  operator-managed deployment choices; no runtime behavior or product
  positioning changes.

## Expected improvement

Documentation only; expected runtime performance change is 0%. The index
provides one starting point across the existing operations guides. No
quantified reduction in search time is claimed because no operator study was
run.

## Validation

- Reviewed the deployment guide inventory and checked index links against the
  repository paths.
- Reviewed the README link and confirmed it points to the new index.
- Documentation-only change; Cargo tests and benchmarks do not apply.
- Outcome: documentation link review passed; no runtime regressions are
  applicable.
