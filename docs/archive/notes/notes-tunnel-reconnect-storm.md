# Tunnel reconnect storm protection

## Findings and plan

The client reconnects physical tunnels independently per lane: each lane has a
connect mutex, consecutive failures, and a fresh random 80–120% multiplier on
its exponential retry delay. This already spreads simultaneous lane retries
without coordinating lanes or changing the authenticated TCP/yamux protocol.
The delay calculation capped the jittered result at 16 seconds, however, so
failure counts whose base delay reached that ceiling all collapsed to exactly
16 seconds. That weakened the spread during a prolonged outage.

The reference guide lists Hysteria2 and quic-go for retry and loss-handling
ideas. This change uses only the general bounded exponential backoff with
jitter pattern; it does not adopt their transport protocols or alter the
project's no-impersonation, small-operations positioning. Preserve the 16
second maximum while capping the base delay at 13,333 ms before applying the
existing 80–120% random multiplier. This retains a meaningful retry window at
the ceiling and needs no new dependency or config option.

Expected effect: at and beyond the largest backoff step, independent lanes
remain spread over roughly 10.7–16 seconds instead of converging at exactly
16 seconds. Initial connects remain immediate, and healthy-path throughput is
unchanged.

## Implementation and experiment

Adjusted the base-delay ceiling to leave room for the existing jitter and added
deterministic boundary assertions showing distinct delays at high failure
counts, alongside the existing exponential, jitter, and hard-cap checks.

Validation: `cargo test -p espejismo-client` passed all 29 tests (0 failed).

No throughput benchmark applies: only outage retry scheduling changes; the
healthy data path and wire protocol are untouched. The relevant improvement is
retry desynchronization, not steady-state throughput.
