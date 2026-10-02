# Performance Tuning Documentation

## Scope And Findings

Documentation-only task. `docs/deployment/CONFIG.md` already listed many
performance-related defaults and abbreviated tuning suggestions, while
`docs/deployment/PROFILES.md` and
`docs/testing/THROUGHPUT_TUNING_HK2_RK.md` documented profile names and selected
benchmarks. Operators still lacked one guide connecting workload choice,
profile trade-offs, relevant knobs, and a repeatable measurement workflow.

Reviewed `docs/research/REFERENCES.md` and `docs/POSITIONING.md` first. The
reference list recommends measured, multi-round comparisons and learning
benchmark methods from projects such as Hysteria2, quic-go, and iperf3. This
guide borrows that measurement discipline only. It preserves Espejismo's native
Rust TCP/Yamux tunnel, authenticated encryption and chaos, non-camouflage
positioning, and small operations model.

## Change And Rationale

- Added `docs/deployment/PERFORMANCE.md` with profile selection by workload,
  tuning workflow, setting-to-effect mapping, memory/latency trade-offs, and
  benchmark interpretation.
- Included implementation-specific caveats: Yamux remains recommended;
  `native_initial_window_bytes` concerns native mux; additional lanes help
  concurrent flows rather than guaranteeing faster single streams; HTTP bulk
  classification does not cover CONNECT/SOCKS5/TUN; and throughput profile
  padding/jitter trade-offs are explicit.
- Cited existing HK2-to-RK median data as an example and cautioned that results
  vary by path. No numeric performance gain is claimed for this documentation
  change.

Expected runtime performance change: **0%**. No executable code, configuration
defaults, or protocol behavior changed. Expected user benefit is clearer,
lower-risk tuning and fewer unsupported assumptions about universal optimal
values.

## Validation And Evidence

This is a pure documentation change, so a before/after runtime benchmark would
measure identical binaries and cannot establish a documentation-related
performance gain. The performance gate is addressed by documenting existing
measured results and making no runtime-performance claim. The referenced
five-round test reported four-way upload medians of 466.8 Mbit/s direct and
459.6 Mbit/s proxied (101% same-window efficiency); historical runs varied,
and the guide labels these as path-specific observations. No runtime
regression is possible from the documentation-only changes.

Reviewed the guide against current config defaults and profile implementation
in `crates/espejismo-core/src/config/mod.rs`, and cross-checked benchmark
instructions against `docs/testing/BENCHMARKS.md` and
`scripts/bench-throughput.sh`.
