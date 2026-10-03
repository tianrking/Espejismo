# Audit logging documentation

## Findings and change

The `[logging]` settings already control the shared tracing output: level,
format, optional file destination, and ANSI behavior. Operational logs contain
some security-relevant events, including authenticated tunnel acceptance and
authentication failures/timeouts. However, the project does not provide a
separate audit subsystem or a complete record of admin requests/configuration
changes. Local file output is also not tamper-evident and rotation/retention is
managed externally.

Expanded `docs/deployment/LOGGING.md` to explain this boundary and give
audit-oriented setup guidance: preserve `info` events, use structured JSON when
appropriate, forward/retain logs through host-managed facilities, and protect
connection metadata in collected records. This describes existing behavior;
it does not change the project's small-operations model or imply a stronger
audit guarantee than the implementation provides.

## Expected result

Operators can configure the existing logging controls for operational evidence
and understand their limitations. No runtime, protocol, or performance change
is expected from this documentation-only update.

## Review / experiment

- Cross-checked `[logging]` fields, defaults, output handling, and file behavior
  against `LogConfig` and `crates/espejismo-core/src/logging.rs`.
- Cross-checked documented event coverage against server authentication and
  admin handler call sites. Admin `/reload` and `/apply` outcomes are not logged
  as a complete audit trail.
- Documentation-only change; cargo tests and throughput benchmarks are not
  applicable.
