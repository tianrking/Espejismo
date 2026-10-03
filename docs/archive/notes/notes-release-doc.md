# Reusable release checklist

## Findings

The repository had checklists for v0.1.3 through v0.1.5, but they repeated
format, lint, test, fuzz, and artifact-matrix gates without covering release
preparation and post-publication review. The actual workflow is tag-driven:
pushes of `v*` tags run the eight-target artifact matrix, generate SHA256SUMS,
and publish a GitHub Release. A `workflow_dispatch` run builds artifacts but
does not publish a release. CI also builds native binaries on Linux amd64,
macOS arm64, and Windows amd64.

## Plan and expected benefit

Added `docs/release/RELEASE_CHECKLIST.md` as the reusable release process. It
covers version and changelog preparation, compatibility and migration review,
the CI validation commands, package/workflow consistency, tag-triggered
publishing, artifact and checksum inspection, and status-document upkeep. It
links the existing packaging, changelog, and version-compatibility guidance.

The expected improvement is fewer missed release steps and less ambiguity about
manual workflow runs versus tag-based publication. This is documentation-only:
no runtime, protocol, performance, or reliability change is expected, so a
percentage improvement is not applicable.

## Verification

- Compared all documented checks with `.github/workflows/ci.yml` and
  `.github/workflows/release.yml`.
- Checked the platform list against the workflow matrix and artifact build
  steps; confirmed full and server-only packages and SHA256SUMS are produced.
- Confirmed relative links target the existing deployment and changelog guides.
- No source code changed. Cargo tests and performance benchmarks do not apply
  to this documentation-only change.
