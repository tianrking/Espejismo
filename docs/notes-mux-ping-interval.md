# Runtime mux ping interval

## Findings and approach

The vendored `tokio-yamux` creates its keepalive timer from `Config` when a
session starts, so changing the configured cadence during a live session was
not possible. The yamux reference listed in `docs/research/REFERENCES.md`
highlights keepalive as session behavior; this change keeps that behavior in
the existing yamux session and does not introduce another protocol or alter
Espejismo's positioning in `docs/POSITIONING.md`.

Added `Session::set_keepalive_interval`, which replaces the timer to apply the
new cadence from the time of the update and updates the session's config. It
uses the same 1 ms minimum as construction, preserves whether keepalive is
enabled, and leaves outstanding ping timeout tracking intact. Expected impact:
runtime configurability for live sessions, with no throughput change or
additional work on the data path.

## Validation

- `cargo test -p tokio-yamux keepalive_interval_can_be_updated_during_a_session --lib --offline` — passed. Checks changing a live session from 30 s to 250 ms, retains an active timer, and confirms a zero update is sanitized to 1 ms.
- `cargo test -p tokio-yamux --lib --offline` — all 45 library tests passed, including existing timeout-boundary and delayed-ACK ping-storm coverage.
- `cargo test -p tokio-yamux --offline` — library tests passed; the command did not report integration-test results in this sandbox run (loopback integration tests may be restricted). No throughput benchmark applies because the change is a correctness/configuration API and adds no data-path work.

The interval timer is restarted on update, so its next tick is scheduled one
full new interval later. Zero intervals are never passed to the timer.
