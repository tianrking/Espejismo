# Log hygiene

## Findings and scope

The logging guide documents application filtering and suppresses dependency
frame dumps. A review of application log fields found no PSK, admin token, or
traffic-key fields being emitted. TUN flow and datagram logs did expose local
and remote addresses, though: every accepted TCP flow was `info` and every
accepted UDP datagram was `debug`, which is noisy at normal traffic rates and
needlessly broadens routine logs with destination metadata.

## Change and expected result

- Lower TUN TCP flow acceptance from `info` to `debug`; keep listener and
  lifecycle messages at `info`.
- Lower per-datagram UDP policy drops, task-limit drops, and acceptance events
  from `debug` to `trace`. Keep failed datagrams at `warn` for actionable
  operational visibility.
- Keep local/remote socket fields available only at the more verbose levels;
  no credentials, keys, or tokens are added to events.

Expected result: default `info` logging emits no per-flow or per-datagram
success/drop records from TUN ingress, and `debug` no longer emits routine UDP
records. This reduces those event volumes to zero at the default level and
requires no data-path work beyond disabled-level tracing checks. No throughput
gain is claimed.

## Experiment

- `$HOME/.cargo/bin/cargo test --workspace --offline` — application crates
  completed successfully (client: 28 passed, core: 110 passed, remote: 10
  passed, yamux unit tests: 23 passed); the workspace command then failed in
  `crates/tokio-yamux/tests/window_update_deadlock.rs` because the sandbox
  denied an OS operation (`PermissionDenied`, code 1) at test line 31. This
  integration test is outside the logging change. No logging-specific test
  exists; the client test binary compiled and passed with the revised events.

This change only alters tracing levels and fields; no throughput experiment is
applicable and no performance gain is claimed.
