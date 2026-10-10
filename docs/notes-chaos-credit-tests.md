# Randomized Yamux receive-credit tests

## Findings and approach

`tokio-yamux` already calculates advertised receive credit with checked
conversion and subtraction, and commits the increased receive window only
after the update frame enters the session queue. Existing tests target selected
boundary and failure cases. The missing coverage was a longer series of valid
data arrivals, application consumption, and batched window updates.

Following the bounded receive-credit model documented by quic-go (see
`docs/notes-quic-interop-notes.md`), this adds a deterministic, seeded random
sequence against the existing Yamux stream state machine. It preserves Yamux
framing, the existing half-window batching rule, and Espejismo's native
encrypted-tunnel positioning. No protocol behavior or dependency changes are
needed. Expected effect: no runtime or throughput change; improved regression
coverage for interactions among valid credit transitions.

## Changes

- Added a 1,000-step seeded sequence to the `tokio-yamux` unit tests. It
  alternates bounded incoming data with application consumption, invokes the
  production update method, and checks emitted update lengths, update
  thresholds, and `buffered bytes + receive credit <= maximum` throughout.
- Retained the existing explicit tests for malformed receive state, failed
  update enqueue, and send-side overflow.

## Verification

- `cargo test -p tokio-yamux --lib -- --test-threads=1`: 38 passed, 0 failed.
  The randomized test covers 1,000 transitions with a fixed seed; it checks
  both below-threshold no-op and emitted window-update paths, along with the
  credit bound after each transition.
- `cargo test -p tokio-yamux -- --test-threads=1`: all 38 unit tests passed.
  The existing `window_update_deadlock` integration test could not start because
  the sandbox denied its `127.0.0.1:0` bind with `PermissionDenied` before the
  transfer ran. This is an environment limitation; the socket transfer remains
  unverified here.
- This is correctness-only work. No performance claim or benchmark is
  applicable.
