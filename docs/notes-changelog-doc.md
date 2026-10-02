# Changelog maintenance rules

## Findings

The root `CHANGELOG.md` already keeps `Unreleased` above tagged releases and
records released work in `Added`, `Changed`, and `Fixed` sections. It did not
state what belongs in the log, how to write user-facing entries, how to support
performance claims, or how to prepare a release. Existing release checklists
cover build and release gates but do not define changelog upkeep.

## Plan and expected benefit

Added `docs/development/CHANGELOG.md` as the maintenance guide and linked it from
the root changelog. The guide defines audience and scope, focused entries,
optional change categories, evidence for performance claims, and release-time
promotion of `Unreleased` entries. This should make release notes more consistent
and reduce stale, implementation-only, or unsupported claims. The expected gain
is documentation quality and release clarity; no runtime or performance gain is
expected.

The guidance fits the existing small-operations positioning: it asks authors
to surface user-visible configuration and compatibility effects, without
changing product scope or behavior.

## Verification

- Compared the guide's section names and version layout with the existing
  changelog conventions and release checklist.
- Checked relative links from both documents and confirmed the working tree
  contains only the intended documentation and commit-message files.
- No code behavior changed; performance measurements and cargo tests do not
  apply to this documentation-only change.
