# WebSocket Upgrade Boundary Checks

## Change and rationale

The custom HTTP/1.1 Upgrade parser previously ignored malformed header lines,
silently replaced duplicate fields, and accepted any nonempty WebSocket key.
The client also accepted status lines by a `101` prefix and did not verify the
response's `Upgrade` and `Connection` fields. These cases can make peers
disagree about whether the byte stream has switched to WebSocket framing.

The parser now rejects malformed field names, malformed fields, and duplicate
fields. Server requests require HTTP/1.1, the configured exact path, version
13, and a Base64 key decoding to exactly 16 bytes. Client responses require an
HTTP/1.1 `101` status and both Upgrade response tokens. The implementation
keeps the project's real WebSocket underlay behavior and does not add protocol
camouflage.

The approach follows the RFC 6455 handshake boundary checks used by established
WebSocket implementations: validate the HTTP Upgrade before interpreting
subsequent bytes as WebSocket frames. This is correctness hardening, so no
throughput gain is expected; it reduces acceptance of malformed/ambiguous
handshakes to zero for the covered cases.

## Validation

- `cargo test -p espejismo-core underlay::tests --lib`: passed, 9 tests. Covers
  valid and wrong-path requests, malformed/ambiguous request headers, invalid
  keys, exact response status handling, required response headers, binary
  roundtrip, and the encrypted handshake over the WebSocket adapter.
- `cargo test -p espejismo-core`: unit output reached the final test group but
  did not terminate in the available run; it was interrupted. Do not interpret
  this as a full package pass. The focused underlay tests completed successfully.
- No performance benchmark was run because this is protocol correctness work,
  not a performance optimization.
