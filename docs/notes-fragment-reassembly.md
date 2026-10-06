# Fragment reassembly boundary coverage

## Findings and approach

The native mux already splits each `NativeStream::poll_write` into DATA payloads
of at most `MAX_PAYLOAD` (256 KiB). `read_frame` rejects declared lengths above
that bound before allocating the payload, and stream reads concatenate DATA
payloads through the existing buffered stream interface. Existing integrity
coverage used small 8 KiB writes, so it did not exercise the split boundary.

The yamux model in `hashicorp/yamux` and the stream-oriented APIs used by
`shadowsocks-rust` both keep logical stream bytes independent from transport
frame boundaries. We preserve that behavior and Espejismo's native authenticated
tunnel; this change adds boundary regression coverage and documents the existing
wire limit without changing framing or runtime behavior.

## Change and expected result

- Roundtrip a DATA frame exactly at 256 KiB and verify a 256 KiB + 1 write is
  rejected.
- Write `2 * MAX_PAYLOAD + 137` bytes in one stream operation and verify the
  peer reconstructs every byte in order across multiple DATA frames.
- Document the native mux payload bound, splitting behavior, and pre-allocation
  oversized-frame rejection in `docs/PROTOCOL.md`.

Expected performance change: none. This is correctness coverage for boundary
handling, with the practical gain that future changes to splitting, framing, or
receive buffering cannot silently corrupt data at the maximum frame size.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core` — passed: 177 unit tests, 1
  config documentation integration test, 4 HTTP proxy integration tests, and 1
  doctest; zero failures. The new frame-boundary and cross-frame reassembly
  cases passed.
- `cargo fmt --all -- --check` reports existing formatting differences in
  unrelated files across the workspace. The two changed Rust files were run
  through `rustfmt` directly.
- No throughput benchmark was run because this change makes no performance
  claim and does not alter the implementation path.
