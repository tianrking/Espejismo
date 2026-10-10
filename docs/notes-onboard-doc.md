# Contributor onboarding guide

## Findings and plan

The repository already has detailed contribution rules in `CONTRIBUTING.md`, a
crate and source map in `docs/CODE-TOUR.md`, and command selection guidance in
`docs/testing/TESTING_GUIDE.md`. New contributors had to discover and assemble
those documents themselves. Adding another full setup or testing procedure
would duplicate canonical guidance and risk drift.

Add `docs/ONBOARDING.md` as a concise sequence: understand product boundaries,
locate code and authoritative references, make a focused change, and prepare
for review. Link to existing guides for detailed setup, checks, protocol,
operations, security, and commit conventions. Add entry points in the README
and development index.

The expected improvement is qualitative: easier discovery of the first useful
reading path and fewer wrong-document or wrong-module starts. No numerical or
runtime improvement is claimed. The guide preserves the positioning in
`docs/POSITIONING.md` and changes no code or behavior.

## Validation

- Reviewed `CONTRIBUTING.md`, `docs/CODE-TOUR.md`,
  `docs/testing/TESTING_GUIDE.md`, `docs/development/INDEX.md`, and
  `docs/POSITIONING.md` as the source material.
- Checked the new guide's relative links and the README/index entry points.
- `git diff --check`: pass.
- Documentation-only change; Rust build, tests, and throughput benchmark do
  not apply. No performance improvement is claimed.
