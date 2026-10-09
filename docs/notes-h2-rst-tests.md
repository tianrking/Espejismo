# HTTP/2 RST_STREAM boundary tests

## Findings and approach

The HTTP/2 underlay delegates wire parsing and stream state to the Rust `h2`
crate. Existing adapter coverage checked a peer `CANCEL` reset, but did not
cover malformed RST_STREAM framing, the unknown error-code boundary, or a
repeated reset. Per RFC 9113 §6.4, RST_STREAM has a nonzero stream ID and an
exactly four-byte error code; error-code values unknown to an implementation
remain valid and should still terminate only that stream.

This follows the project's existing in-memory raw-frame test style for HTTP/2
SETTINGS and PRIORITY. The runtime adapter now includes the h2 reset reason in
its existing reader-stop diagnostic; it still maps stream termination to EOF
for the byte-stream interface. No tunnel protocol or positioning changes.

## Changes and expected effect

- Added RST_STREAM decoder cases for stream zero, an idle stream, and payload
  lengths 3 and 5.
- Extended the peer-reset adapter test to send an unknown error code followed
  by a repeated reset and assert bounded EOF.
- Included `h2::Error::reason()` in the adapter reader-stop debug event.
- Expected throughput change: 0%; the change is protocol diagnostics and test
  coverage only.

## Validation

Validation passed:

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core http2_rst_stream -- --nocapture`:
  1 boundary test passed (zero/idle stream IDs and payload lengths 3 and 5).
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core http2_underlay_closes_reader_after_unknown_peer_reset -- --nocapture`:
  1 adapter test passed (unknown error code, repeated reset, bounded EOF).
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core`: 297 unit tests
  passed, 1 loopback test ignored, 1 config example test, 10 HTTP proxy tests,
  and 1 doctest passed.

The protocol tests use in-memory Tokio duplex transports; no loopback socket is
required. No performance benchmark applies.
