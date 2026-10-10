# Versioning and release branches

This page explains how Espejismo versions releases and keeps release work
aligned with the repository's Git and GitHub Actions workflow.

## Version numbers

The workspace package version in the root `Cargo.toml` is the release version
for the Espejismo client and server. Workspace member manifests inherit it.
Update the workspace version and the matching `Cargo.lock` package entries
together when preparing a release. The separately versioned `tokio-yamux`
dependency is not an Espejismo release number.

Use `MAJOR.MINOR.PATCH` version numbers:

- Increase **MAJOR** for an incompatible public change when the project has a
  stable public interface and the change warrants a major release.
- Increase **MINOR** for a backward-compatible feature release.
- Increase **PATCH** for backward-compatible fixes and maintenance releases.

While the project remains before `1.0.0`, keep releases in `0.MINOR.PATCH`.
Treat changes to configuration, command behavior, and the authenticated wire
protocol as compatibility-sensitive; document their actual effects in the
changelog and compatibility guide. Do not infer protocol compatibility from
the binary version. The handshake protocol version is managed separately.

The release tag must be `vX.Y.Z` and match the workspace release version and
the changelog heading. The release workflow builds on pushes of `v*` tags;
manual workflow dispatch builds artifacts but does not create a GitHub
Release. Follow the [release checklist](../release/RELEASE_CHECKLIST.md) to
validate and publish a release.

## Branch policy

`main` is the integration branch and the source of releases. Merge completed
work into `main`, prepare the version and changelog there, and create the
`vX.Y.Z` tag on the validated release commit. Do not create a permanent
`release/*` branch for routine releases; the current workflow is tag-driven
and does not maintain release branches.

If a release needs stabilization before publication, use a short-lived
`release/vX.Y.Z` branch cut from the intended `main` commit. Keep it limited to
release fixes and documentation, and merge each accepted fix back to `main`
before tagging. Delete the temporary branch after publication. If a released
version must receive a patch while development continues on `main`, make and
validate the fix on `main` where possible, then tag that commit with the next
patch version. Maintain an older release branch only when maintainers
explicitly commit to supporting that line; apply fixes to `main` as well.

Never move or reuse a published version tag. Correct a release by publishing a
new version, with the correction described in its changelog.

## Release records

Keep unreleased user-visible changes under `Unreleased` in the root
`CHANGELOG.md`. At release preparation, verify those entries, move them under
the version heading, and update the workspace version and lockfile. Preserve
versioned release checklists as historical records. See the
[changelog rules](CHANGELOG.md) and the [binary compatibility policy](../deployment/VERSION-COMPATIBILITY.md)
for their respective details.
