# API stability

This page describes compatibility expectations for Espejismo interfaces. It
helps operators and integrators assess upgrades; it does not promise that every
internal Rust module or every pair of releases will remain interchangeable.

## Compatibility boundaries

Espejismo has several interfaces with separate compatibility rules:

| Interface | Compatibility authority | Expectation |
| --- | --- | --- |
| Client and server command line | Release notes and CLI reference | Document changed flags, defaults, output, and exit behavior. Preserve documented behavior when practical; call out breaking changes and migration steps. |
| TOML configuration | Config reference and migration notes | The parser rejects unknown keys. Document removed, renamed, or reinterpreted keys and the required config edits. Do not imply automatic migration or aliases unless implemented. |
| Authenticated tunnel protocol | Protocol specification and version compatibility guide | The handshake currently requires an exact protocol-version match. A wire change must update the protocol contract and version behavior; binary release numbers alone do not promise peer compatibility. |
| Rust crates | Crate rustdoc and release notes | Public items are those documented or re-exported for external use. Before `1.0.0`, source compatibility is not guaranteed across minor releases; review release notes and compile integrators against the target version. |
| Admin HTTP API and metrics | Admin and metrics references | Document routes, authentication, schemas, and metric names. Treat changes that affect external automation or dashboards as compatibility-sensitive and provide migration details. |

Internal modules, undocumented behavior, log wording, and implementation
details are not stable interfaces. They can still matter operationally; if a
change has a user-visible effect, describe that effect in release notes.

## Change expectations

- Prefer compatible additions when they meet the need. Keep configuration
  validation strict and avoid silent reinterpretation.
- For an incompatible change, identify affected versions and users, explain
  the reason, give a replacement or migration steps, and state the target
  release before removal. Follow the [deprecation policy](DEPRECATION.md).
- Update the canonical reference, relevant examples, and changelog together.
  Protocol changes also update [the protocol specification](../PROTOCOL.md)
  and [client/server compatibility guidance](../deployment/VERSION-COMPATIBILITY.md).
- Do not claim a compatibility window, deprecation warning, fallback, or
  cross-version guarantee unless the project implements and verifies it.
- Security or serious correctness fixes may require a faster break. State the
  reason, affected releases, and operator action in release notes.

## Upgrade planning

Before upgrading, read the target release notes and migration guidance. Validate
a copy of the saved configuration with the target binaries. Unless a specific
client/server pair is documented as compatible, plan a coordinated upgrade;
the release version does not negotiate protocol compatibility. For release
numbering and pre-`1.0.0` policy, see [versioning](VERSIONING.md).
