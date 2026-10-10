# WebSocket close handshake boundary tests

## Findings and approach

`underlay.rs` already validates close payload length, status-code ranges, and
UTF-8 reasons, echoes peer CLOSE, and turns application-side shutdown into an
empty CLOSE followed by wire shutdown. The close reader intentionally has no
standalone timer; callers bound handshake IO with their existing timeout. The
RFC 6455 handling style in Xray-core and sing-box, referenced from
`docs/research/REFERENCES.md`, supports testing the frame state and adapter
shutdown paths while keeping this as a real WebSocket underlay. No transport
or project positioning changes are needed.

Added regression coverage for status-code range edges, caller-bounded timeout
on an incomplete CLOSE frame, and application half-close ordering (CLOSE then
wire EOF). These tests do not require loopback sockets. Expected gain is
correctness coverage only; no throughput improvement is claimed.

## Validation

- `cargo test --offline -p espejismo-core websocket_`: passed, 20 matched
  tests, including the new code-range, timeout, and half-close cases.
- `cargo test --offline -p espejismo-core`: the run did not finish in the
  available window. It passed 308 unit tests with the existing loopback-bind
  test ignored, then stalled at `mux::native::frame::tests::arbitrary_bytes_are_panic_free`;
  the run was interrupted. This is not counted as a full package pass.
- No benchmark was run because this is protocol correctness work.
