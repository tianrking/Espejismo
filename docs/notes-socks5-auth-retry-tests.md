# SOCKS5 authentication retry boundaries

## Findings and scope

SOCKS5 RFC 1929 authentication is handled per connection by
`ingress::socks5::verify_password_auth`. A failed attempt sends the failure
status and returns an error, ending that request handler. The SOCKS5 ingress
has no retry counter, delay/backoff, or account/source lockout state. Adding
those policies would require choices about identity, thresholds, expiry, and
bounded state, so this change tests the existing connection boundary without
inventing policy. This follows the narrow protocol-boundary approach used by
sing-box and shadowsocks-rust; it does not change Espejismo's positioning or
add a dependency.

## Changes and expected impact

- Added an in-memory exchange regression proving that a failed credential
  attempt cannot be followed by a successful pipelined retry on the same
  connection, while a new connection can authenticate successfully.
- Added coverage that both RFC 1929 credential fields accept their maximum
  representable length of 255 bytes.
- No throughput change is expected. These tests guard against accidental
  cross-connection lockout and wire-field boundary regressions; no retry delay
  or lockout behavior is promised by the implementation.

## Verification

- `cargo test --offline -p espejismo-core ingress::socks5::tests`: 41 passed,
  including failure-then-fresh-connection behavior and maximum field lengths.
- `cargo test --offline -p espejismo-core`: 313 unit tests passed, 1 existing
  test ignored; config example, all 10 HTTP proxy integration tests, and the
  doc test passed. No loopback test was added.
