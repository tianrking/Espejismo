# Graceful remote restart

## Findings and approach

The remote accept loop previously returned immediately after SIGINT/SIGTERM;
peer handlers were detached Tokio tasks and process exit could interrupt even
handlers already close to completion. The local binary still aborts listener
tasks. Espejismo has no session transfer protocol, and
`docs/ARCHITECTURE.md` explicitly excludes transparent migration of established
proxy flows. This change therefore adds bounded remote connection draining,
not cross-process stream migration.

The reference list points to Yamux for GOAWAY and draining existing streams.
That is a useful model within one mux session, but does not transfer a session
between processes. The server now stops accepting first, waits up to 10 seconds
for peer handlers to return, and aborts remaining tasks after the deadline.
This gives short-lived exchanges a chance to finish while keeping service
shutdown bounded. A fresh process can accept new connections, but interrupted
streams still require application retry.

## Expected effect

No throughput change is expected. Short peer exchanges that finish within the
10-second window can complete during remote shutdown; longer lived tunnels are
interrupted at the deadline. The bounded wait can add up to 10 seconds to
service stop time.

## Verification

- `cargo test -p espejismo-server restart_drain_tests` — passed (2 tests): a
  finishing peer task is joined before shutdown returns, and a pending peer
  task is aborted when the grace period expires.
- The initial socket-level listener replacement test was denied by the
  sandbox (`PermissionDenied` while binding loopback). It was removed; the
  regression tests exercise the shutdown task lifecycle without claiming an
  OS-level reconnection result. Actual client reconnect behavior remains the
  normal fresh-connection path, not migration of an existing stream.
- `cargo test -p espejismo-server` — passed: 32 tests passed, 1 existing
  loopback-dependent relay test ignored, 0 failed. `cargo fmt --check` currently
  reports formatting differences in unrelated workspace files; the modified
  server file was formatted directly with `rustfmt --edition 2021`.
