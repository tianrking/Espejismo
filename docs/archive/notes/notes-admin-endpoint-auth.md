# Admin Endpoint Authentication Audit

## Findings and scope

Admin endpoints expose runtime state, user and traffic counters, lane activity,
and configuration reload/apply operations. The request handler checks the
configured token before reading a request body or dispatching any route, so the
same gate protects `/status`, `/connections`, `/metrics`, `/reload`, and
`/apply`. `/healthz` is also behind that gate. With no token configured, the
handler permits access; configuration validation requires a token for any
non-loopback admin listener, while loopback-only access can remain convenient
for local probes. The listener remains disabled by default.

Token comparison uses constant-time equality. Bearer authorization and the
documented legacy `X-Espejismo-Admin-Token` header are accepted. Existing
configuration validation rejects empty tokens and public listeners without a
token.

## Change and expected result

- Extended the authorization regression test to cover absent credentials,
  unsupported schemes, prefix/suffix token mismatches, and a valid fallback
  header when another credential is wrong.
- No endpoint behavior or product positioning changed. This verifies the
  credential matching boundary used before all route handlers and improves
  confidence that incomplete or malformed credentials do not expose status,
  metrics, or write operations.
- Expected impact: no performance change; authorization runs once per admin
  request and the tests add no runtime cost.

## Experiment

- `cargo test -p espejismo-core --offline admin::tests::authorization_accepts_bearer_and_legacy_header` — passed (1 test).
- `cargo test -p espejismo-core --offline` — passed (120 unit tests, 1 integration test, 1 doctest).
- I attempted a loopback socket test for route-level 401 responses, but this
  environment denies local socket operations (`Operation not permitted`). The
  regression coverage therefore exercises the shared authorization function;
  code inspection confirms it runs before the route match. No throughput
  benchmark applies to this correctness-only test change.
