# Shutdown Drain Documentation

## Findings and approach

Reviewed the client and remote service loops, native mux idle-drain behavior,
the systemd units, deployment runbook, and project positioning. Both binaries
select on Ctrl-C/SIGINT and Unix SIGTERM. The remote stops accepting work in its
main loop but does not join active peer handlers; the local aborts listener
tasks. Neither implements a process-wide grace period or waits for active
tunnels to drain. Client reconnect does not resume an in-flight stream.

The existing `native_drain_timeout_secs` is scoped to GOAWAY after an idle
native mux session, not process shutdown. The docs now make that distinction
explicit and give production operators a stop order for load-balanced and
single-instance deployments, along with the limits of a systemd stop timeout.
This remains documentation-only and preserves the project's small operations
model and protocol behavior.

Expected runtime performance improvement: 0% (no runtime change). The
operational benefit is clearer outage planning and fewer incorrect expectations
about connection continuity; that benefit is not quantified.

## Verification

Cross-checked the statements against `crates/espejismo-client/src/main.rs`,
`crates/espejismo-server/src/main.rs`,
`crates/espejismo-core/src/mux/native.rs`, and the supplied systemd units.
Documentation review passed. No runtime correctness tests or throughput
benchmark apply because no code or performance behavior changed; there is no
performance claim. No regression is possible from these documentation edits.
