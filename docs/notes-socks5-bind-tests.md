# SOCKS5 BIND boundary tests

## Scope and rationale

SOCKS5 ingress supports `CONNECT` and `UDP ASSOCIATE`; `BIND` (command `0x02`)
is intentionally unsupported. The existing parser already responds with
`0x07` (command not supported), but that behavior had no regression coverage.
The new test sends BIND with IPv4, domain, and IPv6 address encodings and with
port zero and `65535`, asserting rejection and reply `0x07`. These checks use
Tokio's in-memory duplex stream, so they need no local socket. This follows the
RFC command-rejection behavior without expanding the proxy's feature scope.

The references index contains no SOCKS ingress implementation to adapt; the
protocol-level response is the relevant design constraint. The SOCKS5 guide
now states explicitly that BIND is unsupported.

This implementation intentionally has no BIND listener, allocated bind
address/port, or second-peer accept timeout. The boundary test therefore also
places a one-second ceiling on each in-memory request exchange, guarding that
unsupported BIND is rejected during parsing rather than waiting for those
unsupported phases. This adds no runtime timeout policy.

## Validation

- `cargo test --offline -p espejismo-core bind_requests_are_rejected_for_address_and_port_boundaries`: passed (1 test). Covers IPv4 zero/max ports, domain zero/max ports, and an IPv6 request; verifies every BIND request fails with reply `0x07` within one second.
- `cargo test --offline -p espejismo-core`: passed (284 unit tests, 1 ignored loopback-bind test, 10 HTTP proxy integration tests, 1 config example test, and 1 doctest; zero failures). The ignored listener test is unrelated and remains annotated `requires loopback bind`.

This is a protocol correctness regression test, with no runtime behavior or performance claim. No throughput benchmark applies.
