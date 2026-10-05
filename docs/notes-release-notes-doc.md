# Release notes template

## Findings

The repository maintains user-facing history in the root `CHANGELOG.md` and
already documents concise entry and release-maintenance rules. The reusable
release checklist covers versioning and validation, but authors had no
copy-ready structure for a versioned release draft or an explicit place to
record compatibility, migration, and validation evidence.

## Plan and expected benefit

Added `docs/release/RELEASE_NOTES_TEMPLATE.md` with optional change categories,
compatibility and migration prompts, and an evidence section. Linked it from
the release checklist and changelog guidance. The expected benefit is more
consistent, user-focused release drafts and fewer omissions when compatibility
or evidence matters. This documentation-only change has no runtime or
performance effect; a percentage improvement is not applicable.

## Verification

- Reviewed the template against `docs/development/CHANGELOG.md` and
  `docs/release/RELEASE_CHECKLIST.md` for consistent categories and policy.
- Confirmed the relative links point to the new template and existing release
  guidance. No source code changed, so Cargo tests and performance benchmarks
  do not apply.
