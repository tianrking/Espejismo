# Dead code and dependency sweep

## Findings and plan

Reviewed workspace manifests and Rust sources, then ran `cargo check
--workspace --all-targets`. The only confirmed unused direct dependency was
`thiserror` in `espejismo-core`: no source or test in that crate refers to it.
Removed that manifest entry; Cargo consequently removed the direct edge from
the core package in `Cargo.lock`. The workspace still depends on `thiserror`
through other packages, so the crate remains in the lockfile.

The only explicit `allow(dead_code)` found is on `tokio-yamux`'s wasm mock
module. That module supplies a platform-specific time shim and was left intact.
No private functions were reported dead by the workspace check. This keeps
the sweep limited to code that can be shown unused without changing runtime
behavior or the project's protocol and product positioning.

Expected impact: one fewer direct dependency edge for `espejismo-core`, with
no expected binary-size or runtime change because the dependency was unused.

## Verification

- `cargo check --workspace --all-targets` completed without warnings before
  the manifest edit.
- `cargo test --workspace` passed 34 client tests, 141 core unit tests, 1
  documented-config integration test, 20 server tests, and 30 tokio-yamux
  unit tests. Its final `tokio-yamux` integration test failed before exercising
  behavior: the sandbox denied its local TCP socket operation with
  `PermissionDenied` (`tests/window_update_deadlock.rs:31`).
- The test suite exercised both native and yamux encrypted bulk transfer paths
  in core. A second run of `cargo test --workspace --exclude tokio-yamux`
  passed, including the core doctest. No behavior changed in this
  dependency-only cleanup.
- No performance benchmark was run because this change removes an unused
  manifest edge and makes no performance claim.

## Conclusion

Removed the unused `thiserror` direct dependency from `espejismo-core`.
All workspace unit, core integration, and doc tests passed. The sole
`tokio-yamux` TCP integration test could not run under this sandbox, so the
full workspace command exits unsuccessfully for that environment limitation.
No runtime or protocol changes were made.
