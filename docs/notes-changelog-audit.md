# Changelog audit

## Findings

Compared the root `CHANGELOG.md` with the entry and release-maintenance rules in
[`docs/development/CHANGELOG.md`](development/CHANGELOG.md) and the shared
formatting guidance in [`docs/DOCUMENTATION_STYLE.md`](DOCUMENTATION_STYLE.md).
The `Unreleased` section was above the versioned history and its prose was
readable, but it had no category heading. Its entries were formatted as a mix of
bullets and fragments, and the 1 MiB-to-64 MiB Yamux integrity-test expansion
was internal test maintenance rather than a user-visible change. The changelog
rules say to omit such internal work.

## Plan and expected benefit

Grouped current user-visible additions under `### Added`, rewrote the fragments
as concise user-facing entries, and removed the internal-only test item. Kept
the adaptive-throughput helper because its RTT threshold and applied profile
are externally observable behavior. No compatibility or project-positioning
claims changed.

This documentation-only cleanup makes the unreleased history conform to the
repository's category and audience rules. Runtime, performance, and reliability
impact: none; a numeric improvement is not applicable.

## Validation and outcome

- Confirmed `Unreleased` remains above the versioned entries and now uses an
  applicable category heading.
- Checked each retained item describes an addition, and removed the internal
  test-maintenance item. No performance or reliability gains are claimed.
- Reviewed relative links in this note against the repository paths; both
  destinations exist.
- No source code or runtime behavior changed, so Cargo tests and throughput
  benchmarks do not apply. No runtime regression is expected.
