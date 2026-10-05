# Error handling audit: hot paths

## Scope and findings

Reviewed production Rust sources under `crates/` for `unwrap`, `expect`, `panic!`, and `unreachable!`; test modules, examples, and benches contain many assertion-oriented panics and are outside the runtime hot-path scope. The actionable runtime occurrence found in the inspected native mux frame handler was conversion of a PING payload to `[u8; 8]` with `expect` after checking its length. The check makes the conversion safe today, but ties safety to control flow and turns a future refactor into a process panic.

The tokio-yamux runtime occurrences are either state-machine assertions protected by retained internal senders / matched state guards, or test-only code. They remain unchanged because replacing those assertions needs a protocol-level policy for impossible channel closure and does not improve malformed-input handling. Other production `expect`s found during the scan are static constants/defaults or bounded cryptographic output with documented invariants; they are not per-frame untrusted-input conversions.

## Change and expected benefit

Replaced the native mux PING conversion panic with explicit conversion error propagation. This keeps malformed frame handling within the existing `Result` path and prevents an invariant regression from terminating the process. No throughput change is expected; the branch is on PING control frames and normal-path cost is negligible.

This audit is intentionally scoped to runtime hot paths. Test assertions and examples retain `unwrap`/`panic` for concise failure diagnostics.

## Validation

- `cargo test --workspace --offline`: unit tests passed across all crates, but the integration test `tokio-yamux/tests/window_update_deadlock.rs` could not bind its loopback listener (`PermissionDenied` from the sandbox). This is an environment restriction; the integration scenario did not execute.
- `cargo test -p espejismo-core --offline`: 109 passed, 0 failed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: unavailable because `cargo-clippy` is not installed for the stable toolchain (`rustup component add clippy` would be required).

No performance claim is made. The correctness path is covered by the full core unit suite, including native mux PING RTT and frame parser tests. The workspace integration test and clippy gate remain unverified in this environment.
