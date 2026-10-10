# API and configuration deprecation policy

## Findings and plan

The repository already documents pre-1.0 versioning, strict rejection of
unknown TOML keys, changelog categories, and client/server protocol
compatibility. It did not provide one procedure for deprecating and removing
Rust APIs, configuration keys, CLI options, or protocol behavior. In
particular, documentation must not promise config warnings or protocol
fallback that the current implementation does not provide.

Added `docs/development/DEPRECATION.md` with staged guidance: identify affected
users, publish a reason/replacement/target release, use Rust deprecation
attributes where applicable, document config and protocol migration honestly,
and update changelog and release material at removal. The policy leaves the
pre-1.0 support interval flexible: aim for one release cycle when practical,
state the target release, and explain shorter security or correctness
exceptions. Linked the guide from the development index.

This should reduce upgrade surprises and inconsistent compatibility claims.
The change is documentation-only and has no runtime or performance effect; a
percentage improvement is not applicable.

## Verification

- Compared the policy with `docs/development/VERSIONING.md`,
  `docs/deployment/VERSION-COMPATIBILITY.md`,
  `docs/development/CHANGELOG.md`, `docs/PROTOCOL.md`, and the release
  checklist.
- Checked that the new relative links target existing files and that the
  policy does not claim runtime warnings, compatibility fallback, or a fixed
  deprecation period unsupported by current project policy.
- No Rust source or behavior changed; compilation, tests, and benchmarks do
  not apply.

## Conclusion

The deprecation workflow now covers public APIs, configuration, CLI, and wire
behavior while preserving strict config parsing and the existing compatibility
model. No runtime or performance change is claimed.
