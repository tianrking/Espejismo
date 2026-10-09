# HTTP/2 SETTINGS boundary tests

## Findings and approach

`underlay.rs` delegates SETTINGS parsing and peer-setting application to the
Rust `h2` crate. Its decoder enforces the RFC's six-byte entry layout, stream
zero requirement, empty ACK payload, and bounds for `ENABLE_PUSH`,
`INITIAL_WINDOW_SIZE`, and `MAX_FRAME_SIZE`; unknown identifiers are ignored
as extensions. This follows the same protocol-layer delegation used by the
existing PRIORITY and CONTINUATION tests, while keeping Espejismo's real HTTP/2
underlay and encrypted tunnel semantics unchanged. No dependency or runtime
behavior change is intended.

## Change

Added in-memory raw-wire tests for truncated SETTINGS entries, nonzero stream
IDs, ACK payloads, invalid push/window/frame-size values, accepted minimum and
maximum legal values, and ignored unknown identifiers. All tests use Tokio's
`duplex` stream and do not require loopback sockets.

Expected benefit: lock down malformed-peer rejection and legal negotiation
compatibility against future adapter or `h2` dependency changes. No performance
gain is claimed.

## Validation

`$HOME/.cargo/bin/cargo test --offline -p espejismo-core http2_settings_ -- --nocapture`
passed (2 SETTINGS tests). `$HOME/.cargo/bin/cargo test --offline -p
espejismo-core` passed: 290 unit tests, 1 config example test, 10 HTTP proxy
tests, and 1 doctest; 1 loopback test was ignored as required by the sandbox.
The invalid-entry cases all caused connection rejection; boundary and unknown
extension cases left the connection open. No performance benchmark applies to
this correctness-only change.
