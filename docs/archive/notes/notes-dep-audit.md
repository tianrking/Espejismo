# Dependency security audit

## Scope and approach

Audited the workspace `Cargo.lock` with `cargo-audit 0.22.2` on 2026-10-02.
The installed advisory database is the RustSec `advisory-db` snapshot at
`9b3a3b73` (2026-09-30). Refreshing it online was not possible in this
environment: `cargo audit` could not acquire `/root/.cargo/advisory-db..lock`
because the Cargo home is read-only. The scan therefore used the cached database
with `cargo audit --no-fetch --stale`.

## Findings

- Scanned 315 locked crate dependencies against 1,277 advisories.
- Found **0 vulnerabilities**.
- Found 5 informational maintenance/soundness warnings: four unmaintained crate
  advisories and one unsoundness advisory. `atty 0.2.14` accounts for both an
  unmaintained advisory and a Windows unaligned-read advisory; its only paths
  are Yamux's `criterion` and `env_logger` development dependencies. The
  unmaintained `serde_cbor` warning also comes from Criterion's development
  dependency graph. `paste` is used transitively by `tun-rs`'s netlink crates.
  `proc-macro-error2` is listed in the lockfile warning output but does not
  appear in the host dependency tree.
- None of the findings is a reported vulnerability requiring a security
  version update. Replacing maintained transitive dependencies wholesale would
  broaden scope without addressing a known vulnerable package, so manifests
  and `Cargo.lock` were left unchanged.

## Verification and outcome

`cargo test --workspace --offline` compiled all workspace crates. Unit tests
passed (31 client, 130 core, 1 config example, 19 server, and 23 Yamux tests).
The `tokio-yamux` integration test `one_way_bulk_transfer_exceeding_window`
could not bind its local socket: the sandbox returned `PermissionDenied` at
`crates/tokio-yamux/tests/window_update_deadlock.rs:31`. This is an environment
restriction, so the complete workspace test command did not pass in this run.

Conclusion: no known vulnerability was found in the locked dependencies; no
dependency update was justified by this audit. The advisory scan used a cached
database two days older than the audit date because online refresh was blocked.
