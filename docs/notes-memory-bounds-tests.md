# Native mux memory bounds test

## Findings and approach

Native mux limits active streams with `max_streams`, limits unread bytes per
stream with `initial_window_bytes`, and gives each stream a bounded frame
channel (`stream_buffer_frames`). The peer receives more byte credit only as
the application consumes queued data, so aggregate unread payload is bounded
by `max_streams * initial_window_bytes` per receiving session. The default
configuration (256 streams, 8 MiB window) therefore permits a theoretical
2 GiB of in-flight payload; this change tests enforcement and does not change
that operator-selected capacity.

The `yamux` reference listed in `docs/research/REFERENCES.md` uses bounded
stream windows and stream-count limits as its flow-control/resource controls.
The native mux already follows that pattern. The test exercises the same
explicit limits without changing Espejismo's protocol or operational model.

## Changes and expected effect

- Added a comment at native stream creation documenting why the receive window
  is the payload bound even though the frame channel also has a capacity.
- Added a 32-stream regression test with an 8-byte window per stream. All
  streams fill their window, and each additional byte write blocks until the
  receiver consumes data. This verifies the aggregate unread payload bound of
  32 * 8 = 256 bytes for that test configuration.
- No throughput or allocation improvement is claimed; runtime limits are
  unchanged. The expected gain is regression coverage for aggregate
  backpressure under many simultaneously active streams.

## Verification

- `cargo test -p espejismo-core native_mux_bounds_aggregate_unread_data_across_many_streams`
  passed (1 test). Covers filling each configured byte window, blocked writes
  on all 32 streams, and consuming payload from every stream.
- `cargo test -p espejismo-core mux::native::tests` passed (15 tests), including
  stream-count enforcement, per-stream send-queue bound, window backpressure,
  frame-size rejection, round-trip transfer, and the new aggregate test.

No performance benchmark was run because this is a correctness and resource
bound regression test, with no runtime behavior or throughput change.
