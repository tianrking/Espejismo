# Code Hygiene: Workspace Clippy and Dead Code

## Scope and findings

The requested baseline is `cargo clippy --workspace --all-targets -- -D warnings`.
On the current `de` workspace checkout it completed successfully with zero
warnings across `espejismo-core`, `espejismo-client`, `espejismo-server`, and
`tokio-yamux`.

There were no actionable compiler or Clippy dead-code diagnostics to remove.
The only explicit `#[allow(dead_code)]` found is on
`crates/tokio-yamux/src/session.rs`'s `wasm_mock` module, gated to non-browser
WASM targets. Its mock `Instant` helpers support the platform-specific timer
implementation. Removing the allowance without building that target would be
speculative, so the compatibility code remains intact.

## Plan and expected benefit

Keep this change narrowly scoped: record the warning-free workspace command and
the dead-code audit, without rewriting already clean modules or deleting
conditional platform support. This makes the hygiene result reviewable and
provides a reproducible gate for follow-up changes. The expected improvement is
zero new warning debt; there is no runtime or throughput change expected.

## Verification

- `cargo clippy --workspace --all-targets -- -D warnings`: passed; zero
  warnings.
- `cargo test --workspace`: client (28), core (109), server (10), and
  tokio-yamux unit tests (23) passed. The integration test
  `tokio-yamux/tests/window_update_deadlock.rs::one_way_bulk_transfer_exceeding_window`
  could not run successfully in this sandbox: its socket setup failed with
  `PermissionDenied (OS error 1, Operation not permitted)` at line 31 before
  exercising the transfer. The workspace test command therefore exited
  non-zero due to the environment restriction; no functional regression was
  observed in the executed unit tests.

The hygiene objective is met for the available host targets: full workspace
Clippy is clean, and no safely removable dead code was identified. The
non-browser WASM target was not built in this environment.
