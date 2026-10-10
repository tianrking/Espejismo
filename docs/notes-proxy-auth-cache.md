# Proxy authentication boundary tests

## Findings and scope

The local SOCKS5 and HTTP proxy ingress paths call `ProxyAuth::matches` for
each authentication attempt. There is no proxy-auth result cache, so there is
no cache key, expiry, or invalidation behavior to exercise. Adding a cache
would retain credential-derived state without an identified repeated-work
bottleneck, so this change stays with the existing request-time authentication
model. `ProxyAuth::matches` uses `subtle::ConstantTimeEq` for equal-length
values; unequal lengths are rejected before comparison.

This follows the small, explicit authentication boundaries used by
shadowsocks-rust's protocol handlers while preserving Espejismo's own local
proxy and authenticated-tunnel separation. No protocol, positioning, or runtime
configuration changes are intended. Expected performance change is zero; the
benefit is regression coverage for exact matching, byte and length boundaries,
and an explicitly configured empty password.

## Validation

- Added unit coverage for valid credentials, case changes, embedded NUL bytes,
  short and long inputs, empty input, and matching an empty configured password.
- `cargo test -p espejismo-core ingress::auth::debug_tests --offline`: passed,
  3 tests, including both new boundary cases.
- `cargo test -p espejismo-core --offline`: passed; 199 unit tests passed,
  1 loopback-bind test ignored, the doc config integration test, 5 HTTP proxy
  integration tests, and 1 doctest all passed.

No benchmark was run: this is correctness coverage and introduces no hot-path
work or performance claim.
