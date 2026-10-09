# CORS boundary tests

## Findings and scope

The admin HTTP endpoint has no CORS implementation or `Access-Control-*`
response headers. Its deployment guide defines it as a trusted management
endpoint, with `/healthz` as the only unauthenticated route and all other
routes protected when an admin token is configured. Browser CORS support would
expand access semantics without a product requirement, so this change keeps
the existing policy and adds regression coverage for its boundaries.

## Change and expected effect

- Added a duplex-stream test covering ordinary, `null`, and wildcard `Origin`
  values on a protected route, a browser `OPTIONS` preflight, and an `Origin`
  on the public health route.
- The test confirms protected requests remain unauthorized and no response
  emits `Access-Control-*` headers. The health response remains successful,
  without granting browser-readable cross-origin access.
- Updated the admin deployment guide with the same policy. No runtime behavior
  changed; expected performance change is 0%.

## Verification

- `cargo test --offline -p espejismo-core cors_origins_and_preflight_do_not_grant_admin_access`: passed (1 test).
- `cargo test --offline -p espejismo-core`: passed; 258 unit tests, 1 ignored,
  1 config example integration test, 10 HTTP proxy integration tests, and 1
  doctest passed. The ignored unit test is unrelated to this change.
- The new cases use Tokio duplex streams and require no loopback bind.
- Conclusion: no regressions observed; the cross-origin boundary cases pass.
