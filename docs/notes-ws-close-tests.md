# WebSocket close frame boundaries

The WebSocket frame reader previously treated every CLOSE opcode as EOF and
discarded its payload. That accepted malformed close frames, including a
truncated one-byte status, reserved status values, and invalid UTF-8 reasons.

The parser now accepts an empty payload or a two-byte valid close status with
an optional UTF-8 reason. It rejects reserved/unassigned status ranges and
malformed reason text while preserving the existing 125-byte control-frame
limit. This is a protocol correctness change, with no throughput claim.

The approach follows the explicit control-frame handling and validation
expected from the transport implementations referenced in
`docs/research/REFERENCES.md` (Xray-core and sing-box), while retaining the
real WebSocket underlay described by `docs/POSITIONING.md`; it does not add
camouflage or change the tunnel protocol.

Expected impact: malformed close payloads now terminate frame parsing with an
error instead of being silently accepted. Valid empty close frames, the
minimum status-only payload, and the maximum 125-byte payload remain accepted.

Validation:

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core` — passed: 248
  unit tests, 10 HTTP proxy integration tests, 1 config example test, and 1
  doctest; 1 existing loopback-bind test is ignored in the sandbox.
- `websocket_close_frame_boundaries_validate_payload` — passed; covers empty,
  status-only, maximum-size close payloads, one-byte truncation, reserved
  status 1005, invalid UTF-8 reason, and over-limit extended payload.
- Tests use in-memory Tokio duplex streams and do not require loopback binds.
