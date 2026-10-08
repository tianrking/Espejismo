# SOCKS5 authentication boundary tests

## Findings and change

The SOCKS5 ingress already selects username/password whenever local credentials
are configured, rejects no-auth-only and unsupported method offers, and checks
nonempty credentials before parsing a request. Existing tests covered those
paths, including a mixed no-auth/password offer. The missing authentication
boundaries were a zero-length method list and an unsupported RFC 1929
username/password subnegotiation version.

Added regression tests asserting that an empty method list receives
`05 ff` with or without configured credentials, and that an unsupported
subnegotiation version receives `01 01` after method `02` was selected. Both
cases also assert that the request is rejected before any SOCKS command is
accepted. This is test-only code work: expected throughput change is 0%; the
benefit is explicit protection against negotiation edge-case regressions. It
preserves the local optional proxy authentication model and does not affect
tunnel authentication or protocol positioning.

## Verification

`cargo test --offline -p espejismo-core ingress::socks5::tests` passed all 25
SOCKS5 tests, including both new boundary cases. `cargo test --offline
-p espejismo-core` passed: 216 unit tests, 1 documented-config integration
test, 8 HTTP proxy integration tests, and 1 doctest; 1 existing loopback-bind
test was ignored under the sandbox rule. The tests cover empty method offers
with both configured and unconfigured auth, invalid RFC 1929 version response,
mixed and unsupported method offers, valid and invalid credentials, and ensure
the request is not accepted after auth failures. No throughput benchmark
applies.
