# C1: BDP-driven per-session yamux stream window, no RTT gate

Date: 2026-09-29. Implements candidate C1 from the throughput bottleneck
analysis (10/12 public-IP directions failed the 0.8-vs-bare-TCP gate).

## Why

Steady-state single-stream throughput is capped by the per-stream yamux
receive window: throughput ~= max_stream_window_size / RTT. The old default
(1 MiB) plus the adaptive boost's 150 ms RTT engagement gate meant
high-bandwidth / low-RTT links (40-130 ms) never got a bigger window, so the
sender sat idle parked on an exhausted send window. Per-frame wire overhead is
under 1% -- the loss was idle time, not bytes.

## What changed

- `crates/espejismo-core/src/config/defaults.rs`:
  `default_native_mux_initial_window_bytes` 1 MiB -> 8 MiB.
- `crates/espejismo-core/src/config/mod.rs`: `adaptive_throughput_floor`
  now assumes 1 Gbit/s (was 500 Mbit/s) and clamps to [1 MiB, 64 MiB]
  (was [1 MiB, 16 MiB]). Tunnel-buffer (2x BDP, capped 32 MiB) and TCP
  buffer (4 MiB) formulas unchanged.
- `crates/espejismo-client/src/adaptive.rs`: every RTT observation now
  recomputes the mux window ungated --
  `max(baseline, floor(smoothed_rtt))` -- when the operator left the window
  at its default. The 150 ms engage / 80 ms release hysteresis still gates
  only the tunnel-buffer and TCP-buffer boosts. `engage()`/`release()` no
  longer touch the mux window.
- The window remains a limit, not an allocation: yamux
  `max_stream_window_size` only bounds `recv_window` accounting
  (`crates/tokio-yamux/src/stream.rs`); streams start at the 256 KiB
  protocol constant and grow via WINDOW_UPDATE. No protocol or wire-format
  change; no obfuscation/padding change.
- Tests: updated fixtures to the 8 MiB default; rewrote
  `low_rtt_never_engages` into `low_rtt_raises_window_without_engaging_buffer_boost`;
  added clamp-bound tests (1 MiB / 64 MiB), 1 Gbit/s RTT-mapping tests
  (50 ms -> 6_250_000, 100 ms -> 12_500_000, 200 ms -> 25_000_000),
  an explicit-config-untouched case, and a falling-RTT monotonicity test.
- Docs: `docs/notes-adaptive-throughput.md` updated to the new default,
  1 Gbit/s assumption, and ungated-window behavior.
- `crates/tokio-yamux/src/stream.rs`: formatting-only churn from
  `cargo fmt` (no logic change).

## Expected gain (from analysis, to be confirmed by experiment)

Window-bound links scale ~linearly with window until the secondary ceiling:
rk->de (~104 ms RTT): 8 MiB -> ~80 MB/s predicted (gate needs 55.4).
rk->gcp (~42 ms): 8 MiB -> link-limited ~41 MB/s (gate needs 32.9).
Should flip all 10 failing directions unless the secondary bottleneck binds
first (then C2: bulk chunk policy).

## Verification (2026-09-29, de, default profile)

- `cargo fmt --check`: pass
- `cargo check --workspace --all-targets --all-features`: pass
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pass
- `cargo test -p tokio-yamux`: 23 unit + 1 integration pass
- `cargo test --workspace --all-targets`: all suites pass
  (25 + 106 + 10 + 23 + 1, 0 failed)

## Open follow-ups

- 12-direction strict-default public-IP throughput matrix (experimental gate)
  still to run after this commits.
- Actual bandwidth estimation instead of the 1 Gbit/s assumption (C3).
- Disambiguation experiment (window-only vs buffer-only) for the ~34 MB/s
  secondary ceiling on rk<->gcp.
