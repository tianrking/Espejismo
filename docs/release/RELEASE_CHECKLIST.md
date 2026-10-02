# Release Checklist

Use this checklist for each version. The version in the release tag and
workspace packages should match. Binary release versions do not imply wire
protocol compatibility; review the [compatibility policy](../deployment/VERSION-COMPATIBILITY.md)
before making or documenting a client/server upgrade guarantee.

## Prepare

- [ ] Choose the version and update the workspace package versions in
  `Cargo.toml` and `Cargo.lock` as needed. Check that the binaries report the
  intended version.
- [ ] Review `CHANGELOG.md`'s `Unreleased` entries against the code and docs.
  Remove reverted or superseded entries, then move the remaining entries under
  `## vX.Y.Z`. Follow the [changelog rules](../development/CHANGELOG.md).
- [ ] Review protocol, configuration, compatibility, and deployment changes.
  Update the relevant user documentation and call out migration steps or
  limitations in the changelog.
- [ ] If the release makes performance or reliability claims, include the
  supporting measurements or regression results and link to them from the
  changelog.
- [ ] Check that package contents and supported platforms in
  [Packaging](../deployment/PACKAGING.md) still match
  `.github/workflows/release.yml`.

## Validate

Run the same Rust checks as CI from the release commit:

```bash
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo check --manifest-path fuzz/Cargo.toml
```

- [ ] Confirm all checks pass and review the CI workflow's native binary build
  results for Linux amd64, macOS arm64, and Windows amd64.
- [ ] Inspect the final diff and confirm the changelog version, workspace
  versions, compatibility notes, and package documentation agree.

## Build and publish

- [ ] Create and push the `vX.Y.Z` tag for the validated release commit. The
  `v*` tag push triggers `.github/workflows/release.yml`; a manual workflow run
  builds artifacts but does not create a GitHub Release.
- [ ] Wait for the full release matrix to finish. Confirm the eight platform
  suffixes are present: `linux-amd64`, `linux-386`, `linux-arm64`,
  `linux-armv7`, `darwin-arm64`, `windows-amd64`, `windows-386`, and
  `windows-arm64`.
- [ ] Verify the workflow publishes full and server-only archives for every
  platform, plus `SHA256SUMS`, to the GitHub Release for the tag.
- [ ] Download representative archives, inspect their contents, and verify
  checksums with `sha256sum -c SHA256SUMS` (or an equivalent SHA-256 tool).
- [ ] Check the published release title, tag, changelog notes, and asset list.
  Confirm the latest release metadata and update links point to the intended
  release.

## Record

- [ ] Update `docs/development/STATUS.md` if its current release target or
  shipped feature summary is now stale.
- [ ] Record release-specific evidence or exceptions in a versioned checklist
  under this directory when the release has requirements beyond this standard
  process.
