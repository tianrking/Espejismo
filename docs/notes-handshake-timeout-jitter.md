# Handshake timeout jitter coverage

## Findings and approach

The server's handshake timeout is a fixed upper deadline. On the client, a
failed handshake is recorded as a lane failure; the next lane connection waits
using the existing capped exponential reconnect backoff with independent
80–120% jitter. The transport references in `docs/research/REFERENCES.md`
recommend bounded retry pacing to avoid synchronized retry storms. This task
adds regression coverage at that existing boundary without changing the
protocol, timeout, retry policy, or Espejismo's positioning.

## Changes and expected benefit

Renamed and clarified the 256-lane sampling test to identify handshake timeout
retries as the scenario under test. It checks every sampled delay is within the
80–120% window, at least 20 of the 41 millisecond delay slots are represented,
and the mean remains within 5% of nominal. This makes accidental removal or
narrowing of independent jitter detectable. No throughput gain is expected;
runtime behavior is unchanged. The expected benefit is stronger regression
coverage against synchronized retries after simultaneous handshake timeouts.

## Verification

`cargo test -p espejismo-client handshake_timeout_backoff_samples_spread_lanes_within_the_jitter_window`
passed (1 test). `cargo test -p espejismo-client` passed all 45 tests,
including the jitter bounds, exponential cap, saturated failure counter, and
the 256-lane distribution regression. The distribution test covers the
80–120% bounds, at least 20 distinct millisecond slots, and a mean between
1,900 and 2,100 ms for nominal 2,000 ms backoff.

`rustfmt --edition 2021 crates/espejismo-client/src/tunnel.rs` formatted the
changed source file. `cargo fmt --all -- --check` still reports formatting
differences in unrelated existing files (including other client/core/server
files); it exits nonzero. No performance benchmark applies because retry
timing and runtime behavior are unchanged.
