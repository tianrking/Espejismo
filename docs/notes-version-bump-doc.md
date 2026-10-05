# Versioning and release branch documentation

## Findings and plan

- The workspace release version is `0.1.5` in the root `Cargo.toml`; client,
  server, and core manifests inherit it. `tokio-yamux` has its own dependency
  version and should not be mistaken for the product version.
- The release checklist already requires the workspace version, release tag,
  and changelog heading to agree. The release workflow runs on `v*` tag pushes;
  a manual dispatch does not publish a GitHub Release.
- Existing compatibility documentation distinguishes binary versions from
  the authenticated handshake's wire protocol version. The new guidance
  preserves that distinction.
- No existing permanent release-branch process was found. Document `main` as
  the source of releases, tags as release points, and an optional short-lived
  `release/vX.Y.Z` branch for stabilization. This records an explicit
  recommended policy without changing the workflow or Git state.

The change adds a canonical development guide and links it from the
development index. This should make version and branch decisions easier to
apply consistently; as a documentation-only change, it has no runtime or
performance effect.

## Validation

- Reviewed the root and member Cargo manifests, release workflow, release
  checklist, changelog rules, compatibility guide, and development index.
- Confirmed referenced documentation files exist and inspected the final diff.
- No Rust code or behavior changed, so compilation and tests are not
  applicable.

## Conclusion

The documentation agrees with the current workspace versioning and tag-based
release workflow. No behavior change or measurable performance change is
claimed.
