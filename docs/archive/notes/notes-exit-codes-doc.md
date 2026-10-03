# Exit codes documentation

## Findings and plan

The local and remote service binaries and the optional HTTP benchmark helper
return `anyhow::Result<()>` from their Tokio `main` functions. There is no
project-defined exit-code enum or explicit error-to-code mapping. Clap handles
CLI parse failures itself, while returned application errors use Rust's
`Termination` behavior. Therefore the operator reference documents the
observable conventional outcomes (`0`, `1`, `2`) and leaves panic and signal
statuses platform-dependent. It also warns automation not to parse diagnostic
text and separates process statuses from SOCKS5/HTTP protocol responses.

Added `docs/deployment/EXIT-CODES.md` and linked it from the deployment CLI
reference and README index. No process behavior or project positioning changes.

## Expected impact

This documentation-only change should help operators distinguish ordinary
configuration/startup failures, invalid CLI invocations, and external process
termination when writing scripts or reading systemd status. No runtime,
throughput, or security improvement is claimed.

## Review and validation

- Inspected both service `main` functions and `espejismo-bench-http`; each uses
  `#[tokio::main] async fn main() -> anyhow::Result<()>` and contains no
  explicit exit-code mapping.
- Inspected the systemd units: both configure `Restart=on-failure`.
- Checked documentation links and terminology against `CLI.md`, `ERRORS.md`,
  and `SYSTEMD.md`.
- Correctness experiment: `$HOME/.cargo/bin/cargo test --workspace` compiled
  all workspace crates; client tests (31), core tests (130), config example
  test (1), server tests (19), and `tokio-yamux` unit tests (24) passed. The
  final integration test `tokio-yamux/tests/window_update_deadlock.rs`
  failed before exercising its transfer because binding its local socket
  returned `PermissionDenied` (`Operation not permitted`) in this sandbox.
  The workspace command therefore exited 101; this is an environment-limited
  test run, not a passing full-suite claim.
- Documentation-only change: throughput benchmark is not applicable; no
  measured performance improvement is claimed.
