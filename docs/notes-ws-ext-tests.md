# WebSocket Extension Negotiation Boundaries

## Change and rationale

This WebSocket byte-stream adapter does not implement negotiated extensions.
As with the real transport adapter boundaries used by sing-box and Xray-core,
the extension layer must not alter the authenticated tunnel bytes unless both
peers implement and negotiate its framing and state. The project's positioning
also keeps WebSocket as a real underlay, not protocol camouflage.

An extension offer is optional to accept. The server now has regression
coverage proving that an unknown extension with parameters and an unsupported
`permessage-deflate` offer can be declined while completing the ordinary
WebSocket upgrade. Its response must not contain
`Sec-WebSocket-Extensions`. Client-side response checks reject any extension
header, including unknown extension names and parameters, and the existing
frame test verifies compressed RSV1 data is rejected when no extension was
negotiated. No extension compression code or dependency is introduced.

## Expected effect

This is protocol correctness coverage, not a performance change. No throughput
gain is expected; the normal extension-free handshake is unchanged. Explicit
decline prevents peers from interpreting frame payloads with incompatible
compression or other extension state.

## Validation

- `cargo test --offline -p espejismo-core underlay::tests::websocket --lib`:
  passed, 22 tests. Covers unknown and parameterized extension response
  rejection, unknown/unsupported request offers declined during a successful
  in-memory handshake, no extension response header, and RSV1 rejection.
- `cargo test --offline -p espejismo-core`: compiled and began all 335 unit
  tests. The output reached a `mux::native::frame::tests::arbitrary_bytes_are_panic_free`
  case but did not complete within the available run; it was interrupted. This
  is not recorded as a full package pass. The focused WebSocket suite passed.
- No benchmark applies to this protocol correctness change.
