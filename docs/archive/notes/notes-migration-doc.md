# Migration Guide Documentation

## Analysis and changes

The deployment docs already explain Espejismo configuration and its own
`espejismo://` client profile format, but do not give users of other proxy
tools a safe migration path. Added `docs/deployment/MIGRATION.md` with a
feature mapping, a staged cutover checklist, a minimal shared TOML config,
validation commands, and explicit import/export boundaries.

The guide emphasizes that credentials and wire protocols are not interchangeable
and that Espejismo needs its own remote endpoint. This aligns with
`docs/POSITIONING.md`: no protocol impersonation and a focused tunnel rather
than a general-purpose multi-protocol suite. The expected improvement is
reduced setup ambiguity and fewer mistaken assumptions during migration; this
is a documentation/usability goal, not a measured performance gain.

## Review and validation

- Cross-checked configuration keys and defaults against
  `docs/deployment/CONFIG.md` and `configs/examples/espejismo.toml`.
- Cross-checked profile import limitations against
  `docs/deployment/PROFILES.md` and CLI commands against
  `docs/deployment/CLI.md` / `docs/deployment/QUICKSTART.md`.
- Cross-checked product claims against `docs/POSITIONING.md` and underlay
  behavior against the configuration reference.
- Documentation-only change; no code behavior changed. Cargo tests and
  throughput benchmarks do not apply. Manual consistency review found no
  unresolved implementation claims.
