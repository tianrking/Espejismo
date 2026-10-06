# Egress Failover

## Findings and plan

The server already iterated over resolved destination addresses after immediate TCP connect errors, but each `TcpStream::connect` could wait indefinitely for a blackholed address. That leaves DNS multi-address failover ineffective for silent drops. The reference list highlights resilient connection handling in Hysteria2 and quic-go; this change applies only a bounded per-address connect attempt to the existing raw TCP egress path. It keeps the project's native tunnel and small operational model unchanged.

Plan: cap each resolved-address TCP attempt at five seconds, then continue to the next allowed DNS result. Keep policy validation ahead of each attempt so a denied address is never contacted. Add deterministic tests for advancing after a connection failure and skipping a policy-rejected address.

## Expected effect

An unresponsive address will no longer block trying later DNS results indefinitely: failover can progress after at most five seconds per prior candidate. On immediately failed or healthy connections there is no added wait. This is a reliability change, not a throughput optimization; no throughput claim applies.

## Implementation and validation

Implemented the bounded TCP attempt and extracted the sequential selection loop so tests can simulate connect outcomes without requiring loopback sockets. Added tests covering first-address failure followed by success and ensuring private-address policy rejection does not invoke the connector.

Validation:

- `$HOME/.cargo/bin/cargo test -p espejismo-server --offline`: passed; 34 tests passed, 1 existing loopback-dependent test remained ignored. The added failover regression confirms the connector advances from a failed first address to a succeeding second address. The policy regression confirms a denied private address is skipped without an attempt.
- The repository-wide `cargo fmt --all -- --check` reports pre-existing formatting differences across unrelated files. `rustfmt --edition 2021 crates/espejismo-server/src/relay.rs` completed cleanly for the changed Rust file.

Conclusion: bounded sequential failover is covered and the server test suite passed. No throughput benchmark applies because this affects only failed egress connection attempts; success path has no extra wait.
