# HTTP/2 RST_STREAM boundary tests

## Findings and approach

The HTTP/2 underlay adapts each h2 request stream to a Tokio duplex stream.
`spawn_http2_io` currently treats an error from `RecvStream::data()` as the
end of the application read side, while a failed `SendStream::send_data`
stops the application-to-peer copy task. The existing round-trip test covers
normal data but did not exercise stream cancellation. H2 RST_STREAM is scoped
to one stream; it must not be mistaken for a connection failure or hang the
adapter reader.

Following the boundary-focused testing style used by the project's Yamux and
H2 PING tests, this change adds an in-memory h2 client/server test that sends
`RST_STREAM(CANCEL)` from the peer after a successful response. The adapter
must then return EOF to its application reader within a bounded timeout. This
uses Tokio `duplex`, so it needs no loopback socket. It does not change tunnel
protocol semantics or the project's positioning.

## Changes and expected effect

- Added `http2_underlay_closes_reader_after_peer_reset` in
  `crates/espejismo-core/src/underlay.rs`.
- Runtime behavior is unchanged; the test locks in the existing reset-to-EOF
  behavior and guards against a stalled read task. Expected throughput change:
  0%.

## Experiment

- `cargo test -p espejismo-core --offline http2_underlay_closes_reader_after_peer_reset -- --nocapture`:
  passed (1 targeted unit test).
- `cargo test -p espejismo-core --offline`: passed (236 unit tests passed,
  1 ignored; 1 config example integration test, 10 HTTP proxy integration
  tests, and 1 doctest passed). The new RST_STREAM test exercised a successful
  response followed by peer cancellation and verified bounded EOF delivery.
- No throughput benchmark was run because this is a test-only runtime change.
