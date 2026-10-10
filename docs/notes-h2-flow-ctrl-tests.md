# HTTP/2 flow-control exhaustion and recovery tests

## Findings and approach

The HTTP/2 adapter delegates frame parsing and credit accounting to the Rust
`h2` crate. Its existing in-memory integration test sends 512 KiB each way with
both initial windows set to 65,535 bytes, which checks recovery when connection
and stream credit are simultaneously constrained. That setup did not isolate
stream-level recovery from connection-level credit.

Following the transport-adapter approach listed for Xray-core in
`docs/research/REFERENCES.md`, this change keeps `h2` as the protocol state
machine and adds a focused in-memory regression. The stream window remains at
65,535 bytes while connection credit is 256 KiB; a 192 KiB transfer must cross
multiple stream windows while staying below the connection limit. No sockets,
protocol changes, or performance changes are involved, and Espejismo's
HTTP/2-underlay positioning remains unchanged.

Expected benefit: deterministic coverage of stream-window exhaustion and
`WINDOW_UPDATE` recovery independently of connection-window exhaustion; no
throughput improvement is claimed.

## Implementation and evidence

- Added `http2_underlay_recovers_from_stream_window_exhaustion` in
  `crates/espejismo-core/src/underlay.rs`. It transfers 192 KiB over the adapter,
  checks the complete byte sequence, and enforces a five-second no-stall bound.
- Existing `http2_underlay_replenishes_exhausted_flow_control_windows` remains
  the regression for bidirectional transfers with both 65,535-byte windows
  exhausted and replenished.
- Focused command: `$HOME/.cargo/bin/cargo test --offline -p espejismo-core
  http2_underlay_ -- --nocapture` passed (7 tests; includes both recovery tests).
- Full command: `$HOME/.cargo/bin/cargo test --offline -p espejismo-core`
  passed: 335 unit tests, 1 ignored pre-existing loopback-bind test, 1 config
  example test, 10 HTTP proxy integration tests, and 1 doctest; no failures.
- No performance benchmark applies to this correctness-only test change.
