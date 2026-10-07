# Ping interval boundary tests

## Findings and change

The bundled `tokio-yamux` session sends periodic Yamux pings and expects a
response before the fixed 30-second timeout. Its timer rejects zero periods,
so session construction and runtime interval updates must sanitize caller
input. The references list points to Yamux keepalive handling as the relevant
upstream pattern; this change keeps that mechanism and the project's existing
TCP/yamux positioning intact.

Named the existing 1 ms minimum keepalive period and extended the boundary
regression test to cover a 1 ns input as well as zero, exactly-minimum, and
above-minimum values. This makes the lower-bound behavior visible in code and
ensures sub-millisecond configuration cannot reach the timer constructor.
Timeout comparisons remain valid at exactly 30 seconds and expire one
nanosecond later. Expected benefit is robustness only: malformed short
intervals are normalized instead of triggering an invalid timer period; the
default cadence and protocol behavior are unchanged.

## Verification

- `cargo test -p tokio-yamux --lib keepalive_timeout_and_interval_boundaries_are_sanitized` — passed. Covers zero, 1 ns, minimum, and above-minimum periods; timeout just before, at, and just after 30 seconds; and session construction with zero.
- `cargo test -p tokio-yamux --lib keepalive_interval_can_be_updated_during_a_session` — passed. Covers runtime changes to a normal period and zero normalization.
- `cargo test -p tokio-yamux --lib` — passed; all 47 unit tests, including delayed ACK and no-response timeout behavior.
- `rustfmt --edition 2024 --check crates/tokio-yamux/src/session.rs` — passed.
- No performance claim applies; the named minimum is compile-time state and
  adds no runtime work beyond the existing sanitization comparison.
