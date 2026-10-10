# Stream window autotune boundary coverage

## Scope and rationale

The receive window starts at the Yamux protocol's 256 KiB initial credit and
may grow up to `max_stream_window_size`. As in the Yamux window management
approach listed in `docs/research/REFERENCES.md`, credit is replenished in
WINDOW_UPDATE frames when enough capacity has become available. The existing
threshold compared against `max / 2`, which rounds an odd ceiling down and can
send an update one byte before the mathematical half-window boundary.

This change compares doubled credit with the ceiling using `u64`, so odd
ceilings trigger at `ceil(max / 2)` without risking multiplication overflow.
It does not change the protocol, wire format, project positioning, or configured
window ceiling. Expected performance impact is neutral: this is a threshold
boundary correction, not a throughput optimization, and no throughput gain is
claimed.

## Tests and evidence

- Added `odd_window_update_threshold_rounds_up_and_respects_ceiling`: with an
  odd 101-byte ceiling, 50 bytes of available credit are held, 51 are sent,
  and subsequent calls cannot increase the receive window beyond 101.
- Existing `window_update_threshold_sends_at_half_window` covers the even
  threshold; `randomized_receive_credit_updates_preserve_window_invariant`
  covers randomized accounting and the configured ceiling.
- `$HOME/.cargo/bin/cargo test --offline -p tokio-yamux`: all 44 unit tests
  passed. The integration test `one_way_bulk_transfer_exceeding_window`
  could not start its local socket pair: the sandbox returned
  `PermissionDenied (Operation not permitted)` at `tests/window_update_deadlock.rs:31`.
  Therefore the package-wide test command is not fully green in this
  environment; this is an execution restriction, not a failed assertion.
- No throughput benchmark was run: this boundary-only correctness change makes
  no performance claim, and the benchmark script requires a running proxy and
  direct endpoint to produce a meaningful before/after comparison.
