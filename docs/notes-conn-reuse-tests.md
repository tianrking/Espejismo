# Connection reuse boundary tests

## Findings and approach

The client reuses a physical TCP/yamux session per lane until the configured
maximum connection age is reached. The age comparison is isolated in
`connection_expired`, making its exact threshold testable without sockets.
The reference list (`docs/research/REFERENCES.md`) points to bounded resource
accounting and explicit lifecycle handling in yamux and other transport
implementations. This change preserves Espejismo's existing TCP/yamux model and
does not introduce a new transport or alter its positioning.

## Changes and expected benefit

- Use `Instant::saturating_duration_since` when checking session age. If a
  supplied/checking instant is earlier than the recorded connection instant,
  the age is treated as zero instead of risking an invalid elapsed-time
  calculation.
- Extend the connection reuse boundary test to cover the future timestamp
  case, alongside never-connected, just-before-expiry, and exact-expiry cases.
- Expected benefit: prevent a future timestamp from being treated as an expired
  reusable connection. This is a correctness/robustness change; no throughput
  improvement is claimed.

## Verification

Ran `$HOME/.cargo/bin/cargo test -p espejismo-client tunnel::tests --offline`.
All 18 tunnel tests passed (0 failed, 0 ignored), including the four connection
age states and existing lane reuse, reservation, and pruning boundaries. The
tests are unit tests and require no loopback socket. No performance benchmark
was run because runtime performance is not intended to change.
