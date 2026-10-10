# Health Check Documentation

## Findings and scope

The optional admin listener in `crates/espejismo-core/src/admin.rs` serves
`GET /healthz` without authentication and returns HTTP 200 with `ok\n`. Route
matching requires the exact GET path. The listener is disabled unless
`admin.listen` is configured; a non-loopback bind requires an admin token.
The endpoint measures process liveness only and carries no tunnel readiness or
traffic status. This task is documentation-only and preserves the project's
small operational model.

## Change and expected result

- Added `docs/deployment/HEALTHCHECK.md` with endpoint semantics, listener
  setup, a local curl command, and Docker Compose/Kubernetes liveness probe
  examples.
- Linked the guide from `docs/deployment/ADMIN.md`.
- Expected result: operators can configure an accurate liveness probe and
  distinguish process availability from tunnel readiness. Runtime and
  throughput change: 0%; no performance effect is expected from documentation.

## Verification

- Cross-checked response, route, authentication bypass, and bind behavior
  against `admin.rs`, `ADMIN.md`, and `CONFIG.md`.
- `git diff --check` — passed.
- Cargo tests and throughput benchmarks are not applicable: no code or runtime
  behavior changed.
