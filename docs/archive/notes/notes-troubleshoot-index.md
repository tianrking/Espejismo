# Troubleshooting Index

## Changes and rationale

- Added a symptom-based guide index to the existing root `TROUBLESHOOTING.md`,
  linking startup/configuration, connectivity, egress/DNS, outages, TUN,
  logging, admin/health, and known-issue/performance topics to their maintained
  operational guides.
- Updated the README operations table to expose the high-level index while
  retaining a direct link to the detailed deployment checklist.
- Kept troubleshooting instructions in their existing guides rather than
  duplicating them. This is navigation only and does not change the two-binary,
  one-config operational model or project positioning.

## Expected impact

Documentation only; expected runtime performance change is 0%. The index should
make it quicker to select the relevant guide by symptom. No quantified
reduction in troubleshooting time is claimed because no operator study was
run.

## Validation

- Confirmed each new guide link targets an existing repository file.
- Reviewed the entry point and terminology against the README, deployment
  index, and `docs/POSITIONING.md`.
- No Rust code or executable behavior changed; Cargo tests and performance
  benchmarks do not apply. No runtime regression is expected from this
  documentation-only change.
