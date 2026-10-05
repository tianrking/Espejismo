# API and configuration deprecation policy

This guide gives maintainers a consistent way to retire public Rust APIs,
configuration keys, command-line options, and protocol behavior. It helps users
plan upgrades while preserving Espejismo's small operating model and explicit
compatibility boundaries.

## Scope and principles

Treat an interface as public when it is documented or intended for use outside
its defining crate. This includes public Rust items, TOML keys and values,
command-line flags and commands, and the authenticated wire protocol.

- Keep a supported interface when a compatible extension solves the problem.
- Record why removal is needed, who it affects, the replacement, and the
  intended removal release before starting the change.
- Do not silently reinterpret or ignore old configuration. The parser rejects
  unknown keys; preserve that behavior unless a reviewed change explicitly
  provides a safe migration diagnostic.
- Do not promise that binary release numbers imply peer compatibility. Follow
  the [client/server compatibility guide](../deployment/VERSION-COMPATIBILITY.md)
  and [protocol specification](../PROTOCOL.md) for wire changes.

## Deprecate an interface

1. Check whether the interface is actually public and identify its users from
   documentation, examples, configuration fixtures, and code references.
2. Document the reason, impact, replacement or migration steps, and a target
   removal release. If no replacement exists, say so and explain the
   consequence for users.
3. For a public Rust item, use Rust's `deprecated` attribute with a short
   reason and replacement where practical. Keep the old item working during
   the announced period unless a security or correctness issue requires faster
   removal.
4. For a configuration key or CLI option, document the deprecation and
   migration path. Do not claim that users will receive a runtime warning
   unless the implementation actually emits one. If warning support is
   unavailable, state that users must migrate by the announced removal
   release; never accept an alias that changes behavior ambiguously.
5. For protocol behavior, describe the affected peers, handshake or wire
   version impact, rollout order, and whether coordinated client/server upgrade
   is required. Do not imply fallback or mixed-version support without
   implementation and evidence.
6. Add a concise `Deprecated` entry to the root `CHANGELOG.md` when the change
   affects released users. Put detailed instructions in the canonical guide
   and link to it from release notes.

## Remove the interface

- Check the published deprecation notice and target release. Before `1.0.0`,
  Espejismo does not promise a fixed number of minor releases between
  deprecation and removal. Give users a clear target and, when practical, at
  least one release cycle to migrate. Explain any shorter schedule.
- Remove the old interface and update examples, config samples, tests,
  compatibility guidance, and related documentation in the same change.
- Keep strict configuration validation. If a removed key is encountered,
  the error should identify the key and direct users to release migration
  guidance where that context is available.
- Record the removal under `Removed` or `Changed` in the changelog as
  appropriate, including the affected release, replacement, and migration
  impact. Update release notes and the compatibility guide for protocol or
  client/server effects.
- For security or serious correctness issues, removal may happen sooner.
  Document the reason, affected versions, and required operator action in the
  changelog and release notes.

## Review checklist

- Is the interface public and are its users and affected versions identified?
- Is the reason, replacement, target release, and migration path explicit?
- Do warnings or migration diagnostics described in the docs exist in code?
- Are configuration strictness and protocol compatibility statements accurate?
- Do the changelog, release notes, compatibility guide, examples, and tests
  agree with the change?

See [versioning and release branches](VERSIONING.md),
[changelog maintenance](CHANGELOG.md), and the
[release checklist](../release/RELEASE_CHECKLIST.md) for related procedures.
