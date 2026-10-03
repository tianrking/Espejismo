# JSON log field documentation

## Findings and scope

The logging guide explained that JSON output preserves event fields, but did
not catalog the fields or distinguish formatter metadata from fields emitted
only by particular application events. I reviewed the JSON subscriber setup
and representative local startup, remote startup, authentication, and stream
completion call sites. The local and remote `service started` events have
different fields, and flow timing fields vary by ingress and event.

Document `tracing` JSON record metadata and the application fields used by
current lifecycle, authentication, and flow events. State the field types,
scope, units, and optional-field behavior so collectors do not assume every
record has a fixed schema. No runtime, logging, or protocol behavior changes.

## Expected result

Operators can build queries around the current JSON fields and understand
which records can contain each value. This is documentation-only; no
performance change is expected.

## Review / experiment

- Cross-checked formatter metadata and JSON options against
  `crates/espejismo-core/src/logging.rs`.
- Cross-checked event fields against local and remote startup, authenticated
  tunnel acceptance, and flow-completion call sites.
- Reviewed the Markdown diff and links. No executable behavior changed, so
  cargo tests and throughput benchmarks are not applicable.
