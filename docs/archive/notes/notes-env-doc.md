# Environment variable reference

## Findings and scope

Reviewed `docs/POSITIONING.md`, the CLI and configuration references, both
release installer scripts, the runtime argument definitions, and
`scripts/bench-throughput.sh`. The application binaries currently consume
`ESPEJISMO_PSK` for `--psk`; the remaining `ESPEJISMO_*` inputs are for release
installation or the optional benchmark helper. `ESPEJISMO_AUTH_REQUEST` is
provided by the application to authentication extension child processes, so it
is documented as output rather than a user setting. Cargo package metadata
variables and unrelated test/example process arguments are not deployment
environment variables.

## Changes and expected benefit

Added `docs/deployment/ENVIRONMENT.md` with names, consumers, defaults, and
purpose for the supported runtime, installer, and benchmark variables. Linked
the reference from the configuration guide and root README; the existing
installer section is left in place for the quick parameter summary. This is a
documentation-only clarification: no runtime behavior, protocol, or project
positioning changes. Expected benefit is fewer configuration lookups and less
confusion between application settings and helper-script inputs; no performance
gain is claimed.

## Verification

- Audited `ESPEJISMO_*` references in runtime source, installer scripts,
  benchmark tooling, and current docs against the new tables.
- Documentation-only change; no build or tests were run. No runtime behavior
  changed, so there is no test or performance result to report.
