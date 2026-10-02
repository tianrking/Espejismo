# Stream Flow Control Documentation

## Findings and approach

The runtime uses Yamux by default and also contains a beta native multiplexer.
Yamux's initial per-stream credit is fixed at 256 KiB; the shared setting named
`native_initial_window_bytes` maps to Yamux's maximum receive-window ceiling.
Consumed data restores receive credit and window updates are batched around
half the maximum. Native mux instead starts streams with the configured window,
returns credit after the current received payload is consumed, and separately
bounds queued frames. Neither window is a preallocated buffer or a TCP
congestion window.

This documents the code's existing behavior and links it from the performance
guide. The research reference list identifies HashiCorp Yamux as the relevant
upstream model: its transferable lesson is per-stream credit with batched
updates, not a change to Espejismo's transport or product positioning. No
runtime behavior or configuration changed.

## Expected effect

No performance change is expected. The guide should make window tuning safer
by distinguishing Yamux's initial credit from its configured ceiling and by
explaining the native mux's different semantics and buffering limits.

## Validation

Documentation-only change; no runtime or protocol experiment is applicable.
Checked claims against `crates/tokio-yamux/src/stream.rs`,
`crates/tokio-yamux/src/config.rs`, `crates/espejismo-core/src/mux/mod.rs`,
and `crates/espejismo-core/src/mux/native.rs`. No throughput benchmark or cargo
tests were run because no implementation changed. Manually reviewed relative
documentation links and the resulting diff.
