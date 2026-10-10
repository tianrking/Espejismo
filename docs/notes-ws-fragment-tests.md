# WebSocket Fragment Boundary Tests

## Findings and plan

`espejismo-core::underlay::read_ws_frame` intentionally rejects frames with
`FIN=0`; the protocol documentation already specified fragmented-frame
rejection. Inspection found that it nevertheless accepted a final continuation
opcode as ordinary data, and accepted non-minimal 16-bit and 64-bit payload
length encodings. RFC 6455 requires continuation frames to belong to an active
fragmented message and requires the shortest length form.

Keep the adapter's existing bounded, complete-frame behavior and reject these
invalid boundaries explicitly. Tests use Tokio in-memory duplex streams, so
they require no loopback sockets. No performance claim applies to this
correctness change; expected gain is stricter wire validation and prevention of
misinterpreting malformed continuations as tunnel bytes, with no throughput
change intended.

## Changes

- Reject continuation opcode `0x0` explicitly, including a final continuation
  that previously passed through as application data.
- Reject extended length encodings when a shorter form is required, and reject
  64-bit lengths with the forbidden high bit set.
- Add round-trip coverage at payload sizes 0, 1, 125, 126, 127, 65,535, and
  65,536; test malformed fragmented and non-canonical length boundaries.
- Update `docs/PROTOCOL.md` to state the enforced behavior.

## Verification

- `cargo test -p espejismo-core underlay::tests::websocket --lib`: 12 passed,
  0 failed. Covers binary payload length boundaries, fragmented initial and
  standalone continuation frames, non-canonical lengths, and the existing
  ping, close, handshake, and underlay tests.
- `cargo test -p espejismo-core`: 250 unit tests passed, 1 ignored (requires
  loopback bind), 0 failed; all 10 HTTP proxy integration tests and the config
  example integration test passed; the crate doctest passed.
- No benchmark run: this is a correctness hardening change, not a performance
  optimization.
