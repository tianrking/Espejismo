# Congestion signals

## Analysis and change

The native mux pending-frame scheduler already bounds its aggregate queue,
keeps control frames ahead of data, and limits bulk starvation under sustained
interactive traffic. Its existing hard-limit error is the admission safeguard,
but there was no direct test for queue occupancy watermarks or when an advisory
congestion signal should clear.

The reference list points to quic-go's pacing, ACK, and loss-state design and
to Yamux's window management as implementation references. Those transport
mechanisms do not map directly onto Espejismo's TCP/native-mux queue, so this
change keeps the existing bounded queue and scheduling policy. It adds a
rounded-up occupancy watermark for deterministic checks and an advisory 75%
congestion threshold. When the existing hard limit rejects admission, its
error now identifies queue congestion. The threshold does not change frame
admission or protocol behavior. No throughput change is expected or claimed.

## Validation

- Added `native_pending_frames_report_watermarks_and_congestion_recovery`.
  It covers empty, 50%, 75%, and full queue watermarks; congestion entry at
  75%; congestion reporting on hard-limit rejection; and recovery below the
  threshold as frames leave.
- Focused command `cargo test -p espejismo-core --offline
  native_pending_frames_report_watermarks_and_congestion_recovery` passed (1
  test).
- Full `cargo test -p espejismo-core --offline` passed: 178 unit tests, 1
  config-example integration test, 4 HTTP proxy integration tests, and 1 doc
  test (184 total). These tests cover queue occupancy/congestion boundaries
  along with the native mux regression suite. No performance claim applies.
