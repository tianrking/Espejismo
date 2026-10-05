# EOL Policy Documentation

## Findings and plan

- `docs/deployment/VERSION-COMPATIBILITY.md` explains wire protocol versions
  and safe upgrade pairing, but did not define which releases are supported.
- `docs/SUPPORT.md` directs users to issue reporting without mentioning
  release status. No existing document states a support duration or LTS policy.
- Document the narrow policy consistent with that evidence: support only the
  latest stable release, EOL an older release when a newer stable release is
  published, and make no fixed-duration, LTS, or backport promise. Link the
  policy from both compatibility and support entry points.

Expected improvement: users can identify whether an older installation is in
scope for routine maintenance and know to consult current release notes before
upgrading. This changes documentation only; no runtime or performance change
is expected.

## Changes

- Added the release support and EOL policy to the client/server compatibility
  guide, including pre-release and backport boundaries.
- Linked the policy from the support guide and operations index.

## Verification

- Manually reviewed the policy against current compatibility, support, update,
  and release documentation; no fixed support-window or LTS claim was present.
- Checked the added relative links and heading anchor against the edited files.
- Documentation-only change; build and tests were not run. No benchmark
  applies and no runtime or performance change is expected.
