# Log Noise Audit

## Findings and scope

The server's mux UDP relay emitted a `debug` success event for every relayed
datagram. At verbose application logging levels this scales directly with
packet rate. The yamux session also logged each periodic keepalive ping at
`debug`, once per lane per keepalive interval. Other reviewed info events were
listener, lifecycle, and TCP connection events; they are not emitted for each
payload chunk. Existing TUN datagram events are already at `trace` per
`docs/notes-log-hygiene.md`.

## Change and expected result

- Lower the per-datagram UDP relay success event from `debug` to `trace`.
- Lower routine yamux keepalive ping events from `debug` to `trace`.
- Add a regression test that verifies global `trace` still keeps noisy
  transport dependencies such as `tokio_yamux` capped at `info`.

Expected result: application `debug` no longer produces one UDP success log
per datagram, and routine keepalive logs appear only at `trace`. This reduces
these event volumes to zero at `debug` without changing relay behavior. No
throughput improvement is claimed; no packet-path instrumentation or protocol
behavior changed.

## Experiment

- `cargo test --offline -p espejismo-core -p espejismo-server` — passed:
  core 120 tests, server 12 tests; no failures.
- `cargo test --offline -p tokio-yamux --lib` — passed: 23 tests; no failures.
- Combined package run including yamux integration tests passed all preceding
  unit suites but `tests/window_update_deadlock.rs` failed at line 31 because
  the sandbox denied an OS operation (`PermissionDenied`, code 1). The same
  integration test is outside this logging change; yamux unit tests, including
  the keepalive test, passed independently.

This is a log-level and correctness change; a throughput benchmark is not
applicable.
