# SOCKS5 authentication boundary tests

## Scope and rationale

`ingress/socks5.rs` selects either RFC 1929 username/password authentication or
the no-auth method from the configured local proxy mode. Previously it accepted
zero-length username/password fields at the RFC 1929 subnegotiation boundary.
The implementation now rejects either empty field with the RFC failure status
before comparing credentials. Method negotiation remains configuration-driven:
auth-enabled listeners require method `0x02`; unauthenticated listeners require
method `0x00`.

The regression tests use an in-memory Tokio duplex stream to exercise the wire
exchange, including success, wrong credentials, incompatible method offers, and
empty RFC 1929 fields. This follows the compact protocol-level testing approach
used by Rust proxy implementations such as shadowsocks-rust, while keeping
Espejismo's existing SOCKS5 ingress and configuration model.

## Expected impact

No throughput change is intended. The behavior change rejects malformed empty
RFC 1929 credentials deterministically and locks down both authentication
modes; no performance gain is claimed.

## Validation

- `cargo test -p espejismo-core ingress::socks5::tests --offline`: passed, 10
  tests. The new async cases cover no-auth success, each mode's method mismatch,
  password-auth success and failure, and empty username/password rejection.
- `cargo test -p espejismo-core --offline`: passed, 148 unit tests; the
  documentation config integration target was filtered by the package test
  invocation's normal harness behavior where applicable.

No benchmark was run because this is a correctness and protocol-boundary change,
not a performance optimization.
