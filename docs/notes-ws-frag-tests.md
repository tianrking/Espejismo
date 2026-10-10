# WebSocket Fragment and Size Boundary Tests

## Findings and plan

`espejismo-core::underlay::read_ws_frame` consumes complete binary frames and
rejects fragmented messages. That is the documented WebSocket underlay contract
in `docs/PROTOCOL.md`; adding message reassembly would change protocol behavior
and state management, so this task preserves explicit rejection. The parser
checks configured frame limits before payload allocation. Make the conversion
from the wire's `u64` length to `usize` checked, then cover exact/over-limit
payloads and a fragmented message with an interleaved PING and continuation.

The references list points to sing-box and Xray-core WebSocket transports.
Their transport adapters are useful implementation comparisons, while this
underlay remains a bounded frame-to-byte-stream adapter with no reassembly.
The reference list also stresses that technical borrowing must not alter
Espejismo's positioning; no camouflage or transport change is involved.

## Changes

- Check that a decoded WebSocket payload length fits `usize` before allocating.
- Verify the configured data-frame limit accepts a frame exactly at the limit
  and rejects a frame one byte over before reading its payload.
- Verify a fragmented message is rejected at its first fragment even when the
  wire stream contains an interleaved PING and continuation frame.
- `docs/PROTOCOL.md` already specifies fragmentation rejection and the existing
  parser's non-minimal length handling; no protocol change is required.

## Verification

- `cargo test -p espejismo-core underlay::tests::websocket --lib`: 16 passed,
  0 failed. Includes frame length encodings, fragmentation rejection with an
  interleaved control frame, and exact/over configured frame size limits.
- `cargo test -p espejismo-core`: 288 unit tests passed, 1 ignored (requires
  loopback bind), 0 failed; the config example, all 10 HTTP proxy integration
  tests, and the crate doctest passed.
- No benchmark run: correctness and parser-boundary hardening only; no throughput
  change is intended.
