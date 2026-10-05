# Reconnect smoothness

## Findings and plan

The client already reconnects physical authenticated lanes lazily when a new
logical stream needs one. Its retry delay was derived from the process-wide
`RuntimeState::consecutive_failures`, so an error on one lane could delay an
independent healthy lane. The delay was capped exponentially at 16 seconds but
all lanes retried at the same instant.

The yamux reference listed in `docs/research/REFERENCES.md` favors bounded
keepalive and failure handling at the session level. This change keeps that
session/protocol behavior intact and improves only the client's lane manager:
track consecutive lane errors locally, clear the count after authenticated
connect succeeds, and apply a deterministic lane-specific spread to the
existing capped exponential delay. The spread avoids synchronized reconnect
bursts without adding dependencies, timers, wire changes, or configuration.

Expected impact: a lane unaffected by another lane's failure no longer waits
on that failure's backoff; failed lanes retain the same 0–16 second exponential
retry envelope with up to 20% deterministic spread. This is a recovery latency
and burst-smoothing change, not a steady-state throughput optimization.

## Implementation and verification

Added a unit regression test for zero-delay initial connection, exponential
delay, cap, and lane spread.

Validation on the `de` checkout: `cargo test -p espejismo-client` passed all 26
tests (0 failed); the focused backoff regression also passed. Compilation was
included in the package test run. `cargo fmt` completed. No throughput benchmark
was run because this change affects reconnect scheduling only, not the active
data path; reconnect timing itself is covered by deterministic unit assertions.
No steady-state or wire-protocol behavior changed.
