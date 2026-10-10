# Admin reference audit

## Audit and result

Compared `docs/deployment/ADMIN.md` with the shared admin HTTP handler, admin
configuration validation, and the local and remote reload callbacks. The route
list and methods match the implementation: `GET /healthz`, `/status`,
`/connections`, `/metrics`, and `POST /reload` and `/apply`. The health probe is
the sole route that bypasses authentication, and both supported token headers
are accepted.

One authentication statement was too broad. Authentication is enforced only
when `admin.token` is set. The token is optional on loopback listeners; without
one, requests to the other routes are also unauthenticated. Configuration
validation rejects a non-loopback admin listener without a token. Updated the
reference to state these behaviors and recommend loopback for an unauthenticated
listener.

Reload/apply behavior and the restart-required tables were cross-checked against
the runtime callbacks: both construct and validate a candidate before applying,
reuse startup overrides (and the client's profile import), and report restart
cases for process-owned settings. No further discrepancy was found.

This is a documentation-only correction. No runtime, positioning, or
performance change is expected; no performance gain is claimed.

## Verification

- Manually compared the endpoint/authentication description with
  `crates/espejismo-core/src/admin.rs` (`is_health_probe`, `authorized`, and
  route dispatch).
- Compared optional-token and non-loopback validation with
  `crates/espejismo-core/src/config/mod.rs`.
- Reviewed both reload callbacks in `crates/espejismo-client/src/main.rs` and
  `crates/espejismo-server/src/main.rs` against the reload/apply description.
- Result: corrected the optional-token case; documented route, token-header,
  listener-validation, and reload behavior agree with the implementation.
- No build or runtime test applies to this documentation-only change.
