# SOCKS5 BIND boundary tests

## Scope and rationale

SOCKS5 ingress supports `CONNECT` and `UDP ASSOCIATE`; `BIND` (command `0x02`)
is intentionally unsupported. A BIND implementation would need to allocate a
listener, report its address, accept a second inbound peer, and manage the
second-reply timeout. Adding those behaviors would expand proxy scope, so this
change tests the existing command rejection boundary instead.

The parser returns reply `0x07` as soon as the complete unsupported command is
parsed. Tests cover IPv4, domain, and IPv6 address encodings, port boundaries,
and 32 concurrent independent BIND requests. The bounded in-memory exchanges
show that request handling does not wait for a listener, peer callback, or
second-stage timeout, and that concurrent requests cannot cross-talk. They use
Tokio duplex streams, so no loopback socket is required. This follows the SOCKS
command-rejection behavior without changing Espejismo's tunnel model.

## Validation

- `cargo test --offline -p espejismo-core bind_requests_are_rejected_for_address_and_port_boundaries`: passed (1 test). Covers address forms, zero/maximum ports, and immediate `0x07` rejection.
- `cargo test --offline -p espejismo-core concurrent_bind_requests_are_rejected_without_cross_talk`: passed (1 test). Covers 32 parallel BIND streams and the one-second aggregate completion bound.
- `cargo test --offline -p espejismo-core`: passed (346 unit tests, 1 ignored loopback-bind test, 1 config example integration test, 10 HTTP proxy integration tests, and 1 doctest; zero failures). The ignored listener test requires loopback bind and is unrelated to BIND.

This is correctness-only work. It adds no runtime behavior and makes no throughput claim.
