# Ingress burst regression coverage

## Findings and approach

The SOCKS5 ingress parser handles one accepted connection at a time, while the
client listener dispatches accepted sockets into independent tasks. The current
configuration has no ingress connection rate limiter; `shared.pacing` shapes
transport writes and does not limit incoming proxy connections. The references
list recommends bounded resource use and benchmark discipline (including
iperf3/nuttcp methods), but does not justify inventing a new ingress policy or
configuration knob for this narrowly specified test task. The regression
therefore exercises a concurrent burst at the protocol boundary without
changing runtime behavior or the project's transport positioning.

## Change and expected effect

Added a SOCKS5 test that submits 128 independent CONNECT handshakes concurrently
with distinct ports and verifies every parsed target and success reply remains
associated with its own exchange. This is correctness coverage only; it makes
no throughput or rate-limiting claim and has no expected runtime performance
change. A future actual ingress limiter needs an explicit policy (scope, rate,
burst capacity, and rejection behavior) before implementation.

## Verification

- `cargo test --offline -p espejismo-core concurrent_ingress_burst_keeps_socks_requests_isolated`:
  passed (1 test; 128 concurrent handshakes).
- `cargo test --offline -p espejismo-core`: passed (182 unit tests, 1 config
  example integration test, 4 HTTP proxy integration tests, and 1 doctest).
- `cargo fmt --all -- --check` reports formatting differences in multiple
  pre-existing unrelated files across the workspace. The changed SOCKS5 file
  was formatted directly with `rustfmt --edition 2021`.
