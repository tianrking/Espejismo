# Error message quality: egress diagnostics

## Audit and scope

Reviewed user-facing error paths in the core, client, and server crates, including configuration, proxy ingress, egress policy, and handshake boundaries. Many validation paths already name the setting or failed protocol operation. This pass targets egress policy diagnostics, where host and port denials omitted the target value, and `split_authority` exposed Rust's generic integer parse error without identifying the malformed field.

## Plan, changes, and expected benefit

Keep the existing egress policy and security behavior. Improve only the diagnostic context: identify whether the requested host, port, or resolved socket address was rejected, include the rejected value, and state the required host:port port format for malformed authority input. Add unit assertions for the resulting messages and malformed-port case. This should reduce the steps needed to identify a mismatched allow/block rule from inspecting logs and config to a direct rule/value comparison; no numeric support or performance change is expected.

## Validation

- `rustfmt --check --edition 2021 crates/espejismo-core/src/egress.rs`: passed.
- `cargo test -p espejismo-core --offline`: passed, 115 unit tests and 1 doctest.
- `cargo test --workspace --offline`: all crate unit tests passed, including the relevant core suite, but the pre-existing `tokio-yamux/tests/window_update_deadlock.rs` integration test could not bind its loopback listener (`PermissionDenied: Operation not permitted`) in this sandbox. The socket-dependent test did not execute successfully; this failure is unrelated to the egress changes.
- No performance measurement was run because this is a diagnostic text change with no expected runtime behavior or performance effect.

The regression assertions verify that host and port policy errors contain the rejected value and the relevant config setting, and that malformed target ports name the expected field and format. No policy behavior changed.
