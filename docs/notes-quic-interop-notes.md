# QUIC interop notes

## Scope and findings

This is a source-level comparison, not a QUIC implementation or a claim of wire
interoperability. Espejismo's core transport remains its native encrypted
protocol over TCP/yamux; the positioning documents explicitly rule out adding
QUIC as a core transport or camouflage. QUIC and Yamux have different framing,
handshakes, and flow-control signaling, so a quic-go peer cannot speak directly
to a tokio-yamux session.

The useful shared engineering point is bounded receive credit. quic-go documents
per-stream and per-connection receive limits, including a memory commitment
bound, and only grows its receive window within configured maxima. Its auto-
tuning is QUIC-specific and was not copied. In tokio-yamux, `send_window_update`
computes new credit as `max_recv_window - buffered_bytes - recv_window`; this
relied on an internal invariant and could underflow if state became inconsistent
(panic in checked builds, wrapped credit in optimized builds).

## Change and expected effect

`tokio-yamux` now checks conversion and subtraction when calculating the
window-update delta. Inconsistent state returns the existing invalid-message
error and emits no frame. Valid inputs retain the same threshold and wire
behavior. Expected impact: no throughput change on valid sessions; malformed
internal credit state can no longer produce wrapped credit or a subtraction
panic.

## Regression coverage

Added `invalid_receive_credit_does_not_emit_wrapped_window_update`, which
injects receive credit above the configured maximum and checks for an error, no
credit mutation, and no emitted frame. The existing
`window_update_threshold_sends_at_half_window` test covers the valid update
threshold. The existing `overflowing_window_update_preserves_send_credit` test
covers peer update overflow and unchanged send credit.

## Experiment

- `cargo test -p tokio-yamux --lib`: 31 unit tests passed, including the new
  invariant regression, the half-window threshold, and existing send-credit
  overflow coverage.
- `cargo test -p tokio-yamux`: unit tests passed, but the package run exited
  nonzero when the existing `window_update_deadlock` integration test tried to
  bind `127.0.0.1:0`; the sandbox returned `PermissionDenied` before the test
  could exercise its transfer. This is an environment limitation, so the
  end-to-end transfer result remains unverified here.
- This is a correctness hardening change with no expected wire or throughput
  change on valid state; no performance claim is made.

## References

- quic-go flow-control documentation:
  https://quic-go.net/docs/quic/flowcontrol/ — receive windows bound memory;
  stream and connection receive windows are configured independently, and the
  implementation auto-tunes within configured maxima.
- quic-go stream source:
  https://github.com/quic-go/quic-go/blob/master/stream.go — stream send and
  receive halves share the QUIC stream abstraction and flow controller.
