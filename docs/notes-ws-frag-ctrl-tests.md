# WebSocket Fragment and Control Frame Tests

## Change and rationale

The WebSocket underlay previously rejected every non-final data frame and every
continuation frame. RFC 6455 permits a binary message to span frames, with
control frames such as PING interleaved. The reader now reassembles binary
fragments, responds to PING while a message is in progress, and delivers bytes
to the tunnel only after the final continuation. A CLOSE interrupts and
discards a partial message. Invalid continuation ordering and a reassembled
message larger than `max_frame_bytes` close the app stream without leaking a
partial message.

This borrows the RFC-oriented framing discipline used by mature transport
adapters such as Xray-core (listed in `docs/research/REFERENCES.md`) while
keeping WebSocket as a real underlay. It adds no protocol camouflage, dependency,
or performance optimization; expected throughput improvement is 0%.

## Verification

- `cargo test --offline -p espejismo-core underlay::tests::websocket --lib`:
  passed 27 WebSocket tests, including PING between fragments, CLOSE during
  reassembly, invalid continuation order, and existing masking/control-frame
  boundaries.
- `cargo test --offline -p espejismo-core`: passed 345 unit tests; 1 existing
  loopback test was ignored; the config example integration test, all 10 HTTP
  proxy integration tests, and the doctest passed.
- The new cases use in-memory duplex streams and do not bind loopback sockets.
- No throughput benchmark applies to this correctness change.
