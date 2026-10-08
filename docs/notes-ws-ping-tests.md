# WebSocket PING/PONG boundary tests

The WebSocket underlay uses an in-house frame pump. Before this change it
discarded PING and PONG frames, so peers sending PING did not receive the
required matching PONG. That can cause a proxy or peer keepalive check to time
out even while the tunnel is otherwise active.

The reader now echoes each PING payload in a PONG, including the zero-byte
case. The WebSocket wire writer is shared behind an async mutex so PONG and
application data frames are serialized. Frame parsing rejects fragmented
frames, reserved bits, and control frames whose payload length exceeds the
RFC 6455 limit of 125 bytes. This follows the control-frame handling model in
the standard WebSocket implementations discussed by Xray-core and
sing-box in `docs/research/REFERENCES.md`, while retaining Espejismo's real
WebSocket underlay and authenticated tunnel framing.

Expected impact: no bulk throughput gain is claimed. PING probes now complete
instead of being silently ignored; the normal data path only pays the writer
mutex needed to serialize concurrent control and data frames.

Validation:

- `cargo test --offline -p espejismo-core` — passed: 247 unit tests, 10 HTTP
  proxy integration tests, 1 config example test, and 1 doctest; 1
  loopback-bind test ignored by its existing sandbox annotation.
- `websocket_ping_payload_boundaries_roundtrip_and_reject_oversize` — passed;
  covers empty and 125-byte PING payloads plus rejection of an extended-length
  control frame.
- `websocket_underlay_echoes_ping_payload_as_pong` — passed; exercises the
  adapter's actual PING-to-PONG response and verifies the echoed bytes.
- `cargo fmt --all -- --check` is not clean in the baseline: it reports
  formatting differences in unrelated existing files. The modified Rust file
  was formatted directly with `rustfmt --edition 2024`.
