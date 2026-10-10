# Config file permissions

## Plan and expected benefit

`load_config_file` is the shared loading path for the client and server, so it
checks permissions once before reading. On Unix, any group or other permission
bit (mode `0o077`) emits a warning with the path, effective mode, and a `chmod
600` suggestion. The application still starts: this is an operational warning,
not a new compatibility-breaking validation rule. On non-Unix platforms the
check is a no-op because the Unix mode model does not apply.

The check is deliberately limited to the configuration file itself; it does
not recursively audit parent directories or ownership. This is expected to
make accidental exposure of keys and passwords visible at startup with no
material runtime cost (one metadata lookup per file load).

## Changes

- Added a common permission warning to `load_config_file`.
- Added tests for accepted private modes, group/other access detection,
  restricted-mode config loading, and missing-path safety.

## Experiment

- `cargo test -p espejismo-core config::tests`: passed (29 passed, 0 failed).
- `cargo test --workspace`: core (130), client (31), server (17), yamux unit
  tests (23), and config documentation integration test (1) passed. The final
  `tokio-yamux` integration test `one_way_bulk_transfer_exceeding_window`
  failed before exercising the transfer because its socket setup returned
  `PermissionDenied` (`Operation not permitted`) at
  `crates/tokio-yamux/tests/window_update_deadlock.rs:31`; this sandbox
  limitation is unrelated to config permissions.

The targeted configuration suite has no regressions. The full workspace gate
is not fully green due to the sandbox-restricted integration test.
