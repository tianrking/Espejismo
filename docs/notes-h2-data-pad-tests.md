# HTTP/2 DATA padding boundary tests

## Findings and approach

The HTTP/2 underlay delegates frame parsing to the Rust `h2` crate. The
repository already pins other HTTP/2 frame boundaries with raw frames sent
over Tokio duplex streams. Following that pattern keeps this coverage at the
real HTTP/2 decoder boundary, without adding custom protocol parsing or
changing the underlay's identity as a genuine HTTP/2 transport adapter (as
described in `docs/POSITIONING.md`).

The DATA frame PADDED flag adds a one-byte Pad Length before application data.
The length must fit in the remainder of the frame; valid padding is removed
before DATA is exposed to the application. The tests cover zero padding,
maximum padding that fits, empty application data, and a declared padding
length that exceeds the payload.

## Changes and expected effect

- Added raw-frame tests proving valid padded DATA exposes only application
  bytes and an overlong Pad Length is rejected by `h2`.
- No runtime code or wire behavior changed. Expected throughput and latency
  impact: 0%; this is regression coverage only.

## Validation

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core http2_data_padding -- --nocapture`:
  2 tests passed. This covers zero/max/empty padding and invalid-length reject.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core --quiet`:
  333 unit tests passed, 1 ignored; 1 config-example test passed; 10 HTTP
  proxy tests passed; 1 doctest passed.
- All new cases use in-memory duplex streams and require no loopback sockets.
- No benchmark applies because runtime behavior is unchanged.
