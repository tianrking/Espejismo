# CLI help text quality

## Audit and change

Audited the Clap argument structs for `espejismo-local`, `espejismo-remote`, and
the `bench-http` helper. The application options previously relied on Clap's
generated names, which left the help output without explanations; the main
binaries also had no command examples. Added a concise description to each
option, command summaries, and copyable example invocations. The examples show
common client, server, validation, profile-import, TUN, and benchmark usage.

This is a help-text-only change. It does not alter argument parsing, defaults,
configuration behavior, or the project's positioning.

## Expected benefit

Operators can identify each option and see a starting command directly from
`--help`, reducing the need to cross-reference the CLI guide. No runtime or
throughput improvement is expected.

## Verification

- Inspected generated `--help` output for `espejismo-local`,
  `espejismo-remote`, and `espejismo-bench-http`; all declared options have
  descriptions, and the three help pages render their examples.
- `cargo test --offline -p espejismo-client -p espejismo-server`: 38 passed,
  0 failed (28 local, 10 remote; benchmark helper has no unit tests).
- `cargo test --offline --workspace`: package unit tests passed, including
  core (117), server (10), and tokio-yamux unit tests (23), but the workspace
  command exited 101 when `tokio-yamux/tests/window_update_deadlock.rs`
  attempted a socket operation denied by the sandbox (`PermissionDenied`).
  This integration test is unrelated to CLI help; no code test failure was
  reported in the client or server packages.
- No performance experiment applies to this help-text-only change.
