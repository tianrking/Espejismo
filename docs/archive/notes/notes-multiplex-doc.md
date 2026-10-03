# Yamux Tuning Documentation

## Findings And Plan

Reviewed `docs/research/REFERENCES.md` and `docs/POSITIONING.md` first. The
reference list points to HashiCorp Yamux for window and keepalive practices.
This change uses the repository's bundled `tokio-yamux` implementation as the
source of exact defaults and the Espejismo adapter as the source of effective
runtime configuration. It keeps the TCP/Yamux design and does not add protocol
camouflage or new operational machinery.

The runtime creates Yamux with `Config::default()` overrides: the maximum
stream window comes from `shared.mux.native_initial_window_bytes` (clamped to
256 KiB), and the stream count comes from `shared.max_streams`. Yamux streams
start at 256 KiB and can grow up to that configured maximum. The first field's
name is historical and can mislead operators into thinking it affects only
the native mux. Yamux keepalive remains enabled at the bundled default of 30
seconds; it is independent of Espejismo's encrypted TCP heartbeat.

## Change And Expected Benefit

- Expanded `docs/deployment/PERFORMANCE.md` with effective Yamux window,
  stream-count, and keepalive behavior, including the config-name caveat and
  measured tuning advice.
- Clarified that the 8 MiB default is a maximum window, not the stream's
  initial 256 KiB window. Explained memory trade-offs and that increasing
  stream capacity alone does not improve throughput.
- Expected runtime performance change: **0%**. This is documentation only;
  expected benefit is fewer misdirected tuning attempts and more informed
  high-BDP experiments. No quantitative user-performance gain is claimed.

## Validation And Evidence

Cross-checked statements against `crates/espejismo-core/src/mux/mod.rs`,
`crates/tokio-yamux/src/config.rs`, `crates/tokio-yamux/src/stream.rs`, and
the config defaults. The performance benchmark gate does not apply to runtime
code changes here: a before/after benchmark of identical binaries cannot
measure documentation impact. No correctness behavior changed, so runtime
tests were not run. Result: documentation facts align with the implementation;
there is no runtime regression surface in this change.
