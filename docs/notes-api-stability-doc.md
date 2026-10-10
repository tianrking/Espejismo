# API stability documentation

## Change and rationale

Added `docs/development/API-STABILITY.md` and linked it from the development
index. The guide brings the existing versioning, deprecation, configuration,
admin, and protocol rules together while keeping their compatibility promises
separate. In particular, it states that the Rust API has no source-compatibility
guarantee across pre-1.0 minor releases and that binary versions do not
negotiate wire compatibility.

Expected benefit: contributors and integrators can find the relevant contract
and migration authority in one place, reducing ambiguity during upgrades. This
is documentation-only; no runtime, performance, or resource improvement is
expected or claimed.

## Verification

- Manually checked links and claims against `VERSIONING.md`, `DEPRECATION.md`,
  `docs/deployment/VERSION-COMPATIBILITY.md`, `docs/PROTOCOL.md`,
  `docs/deployment/CONFIG.md`, and `docs/development/INDEX.md`.
- No tests or benchmark apply because no code or runtime behavior changed.
