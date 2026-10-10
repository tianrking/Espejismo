# Code owners documentation

## Findings and plan

The repository had no `CODEOWNERS` file. `CONTRIBUTING.md` described review
expectations but did not identify a default review owner. The GitHub repository
namespace is `tianrking/Espejismo`, so this change uses `@tianrking` as the
default owner for all paths. The owner should be replaced or supplemented when
additional maintainers with review responsibility are established.

Added `.github/CODEOWNERS` with a repository-wide default and linked it from
the contributor guide. The guide explains that the file controls review
routing, should track maintainers with GitHub access, and does not replace
technical review or CI. This is documentation and repository metadata only;
the expected improvement is clearer review routing and less uncertainty about
who should receive ownership requests. No numerical improvement is claimed.

This does not affect runtime behavior or Espejismo's product positioning.

## Validation

Manually checked the GitHub username against the repository's configured
`tianrking/Espejismo` remote and verified the relative link in
`CONTRIBUTING.md` points to the new file. Documentation-only change; no Rust
build or tests apply. No performance change or measured improvement is
claimed. `git diff --check` is the formatting check for this change.
