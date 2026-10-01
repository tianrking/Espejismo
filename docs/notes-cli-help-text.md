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

## Follow-up audit (round 076)

Rechecked the current Clap argument structs against the client and server
startup branches and rendered help pages. Their option names, environment
variable, defaults, command examples, and descriptions matched the observed
behavior. One boundary was missing from the benchmark helper help: its
`--chunk-bytes` input is clamped to 1 KiB–1 MiB before use. Updated that
description so operators can see the effective range.

This text-only correction does not change parsing or runtime behavior. It
reduces confusion for values outside the accepted effective range; no
performance change is expected.

### Verification

- Rendered `espejismo-local --help`, `espejismo-remote --help`, and
  `espejismo-bench-http --help`; confirmed the updated chunk-size range appears
  in the generated output.
- `cargo test --offline -p espejismo-client -p espejismo-server`: 50 passed,
  0 failed (31 client, 19 server; benchmark helper has no unit tests).
- `git diff --check`: passed. No performance experiment applies.
