# HTTP/2 PRIORITY frame boundary tests

## Findings and plan

The HTTP/2 adapter delegates frame parsing to the `h2` crate and does not
interpret HTTP/2 stream weights as tunnel scheduling priorities. The upstream
decoder requires a five-byte PRIORITY payload, rejects stream ID zero at
connection scope, and treats a stream depending on itself as a stream error.
These rules follow RFC 9113 §5.3 and §5.3.1. The repository's reference notes
identify Xray-core's underlay abstraction as a useful architectural comparison;
here the adapter keeps HTTP/2 semantics delegated to `h2` rather than adding a
second priority scheduler.

## Changes and expected effect

- Added in-memory raw-frame regression tests for PRIORITY payload lengths 0,
  4, 5, and 6; stream ID zero; and self-dependency.
- The exact five-byte payload stays accepted, malformed payload lengths and
  stream zero terminate/reject the connection, and self-dependency leaves the
  connection alive as a stream-level error.
- No production behavior or scheduling changed. Expected performance gain is
  zero; this pins protocol boundary behavior and protects future `h2` upgrades.

## Validation

`$HOME/.cargo/bin/cargo test -p espejismo-core --offline` — passed: 241 unit
tests, 1 config-example integration test, 10 HTTP proxy integration tests, and
1 doc test (253 passed); 1 pre-existing loopback-bind test remained ignored.
The four new tests cover valid exact length, both neighboring invalid lengths
plus zero length, stream ID zero, and self-dependency's stream-level handling.
All fixtures use Tokio's in-memory `duplex` transport, so no loopback bind is
needed. No throughput comparison applies because this test-only correctness
change does not modify runtime behavior.
