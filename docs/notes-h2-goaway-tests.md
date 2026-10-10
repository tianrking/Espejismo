# HTTP/2 GOAWAY boundary tests

## Findings and approach

The H2 underlay delegates HTTP/2 framing and connection state to the Rust `h2`
crate. Existing in-memory raw-frame tests cover SETTINGS, PRIORITY, RST_STREAM,
and PING, but not GOAWAY. RFC 9113 §6.8 defines GOAWAY as a connection-level
frame on stream zero with an eight-byte minimum payload: last-stream-id and
error code, followed by optional debug data. The last-stream-id identifies the
highest peer-initiated stream that might have been processed.

Following the existing test style around the `h2` API boundary, the additions
use Tokio duplex transports and raw GOAWAY frames. The references list's
Xray-core underlay abstraction and real HTTP/2 characteristic notes support
keeping this as a genuine H2 transport adapter; no protocol or positioning
change is needed.

## Changes and expected effect

- Test malformed GOAWAY on a nonzero stream and payloads shorter than the
  required eight bytes.
- Test a peer GOAWAY with last-stream-id zero and REFUSED_STREAM, asserting the
  client rejects a newly attempted stream and preserves the connection error
  reason.
- Test server `graceful_shutdown()` and verify a stream attempted afterward
  is rejected as GOAWAY.
- Runtime behavior and throughput are unchanged; expected performance delta is
  0%. This change adds protocol regression coverage only.

## Validation

- `cargo test --offline -p espejismo-core http2_goaway -- --nocapture`: 2
  GOAWAY tests passed (raw-frame framing and peer reason/last-stream-id).
- `cargo test --offline -p espejismo-core http2_graceful_goaway -- --nocapture`:
  1 graceful shutdown test passed.
- `cargo test --offline -p espejismo-core --quiet`: 325 unit tests passed, 1
  ignored; 1 config example test passed; 10 HTTP proxy tests passed; 1 doctest
  passed.

All tests use in-memory duplex streams; no loopback socket is required. No
performance benchmark applies because no runtime or throughput path changed.
