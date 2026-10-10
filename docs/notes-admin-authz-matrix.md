# Admin authorization matrix tests

## Analysis and change

The admin handler has one intentionally public action, `GET /healthz`. The
protected actions are `GET /status`, `/connections`, and `/metrics`, plus
`POST /reload` and `/apply`. Existing tests covered representative paths, but
did not exercise every protected action with both supported credential
formats. Added a route matrix test covering absent credentials, an invalid
Bearer token, a valid Bearer token, and the legacy
`X-Espejismo-Admin-Token` header for all five protected routes. It also checks
that health remains public and that only authenticated reload/apply requests
invoke the action callback. Updated the admin guide to keep its authorization
description aligned with this matrix.

Expected benefit is stronger regression detection for future admin route and
authorization changes; there is no runtime behavior or performance change.

## Verification

- `cargo fmt --all` completed.
- `cargo test --offline -p espejismo-core admin::tests`: 10 passed, 0 failed.
  The new route matrix covered every protected route against missing, invalid,
  Bearer, and legacy-header credentials, along with the public health probe and
  side-effect count for protected control actions.
- `cargo test --offline -p espejismo-core`: 175 unit tests, 1 config-doc test,
  4 HTTP proxy tests, and 1 doctest passed; 0 failed.
- Conclusion: all relevant tests pass; no regressions observed.
