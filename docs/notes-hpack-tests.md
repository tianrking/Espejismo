# HPACK boundary tests

## Findings and approach

The HTTP/2 underlay in `crates/espejismo-core/src/underlay.rs` delegates header
compression to the `h2` crate; Espejismo has no independent HPACK encoder or
decoder. Existing tests covered HTTP/2 byte-stream transport and connection
control but did not assert that unusual yet valid header fields survive the
HPACK path. Following the focused protocol-boundary style of the upstream
`h2` implementation, this change adds an in-memory client/server exchange
checking an empty value, repeated field values, and a 16 KiB value. This does
not change HTTP/2 behavior or Espejismo's non-camouflage positioning.

Expected benefit: no throughput change is intended. The gain is regression
coverage against loss, truncation, or accidental coalescing of those header
boundaries; performance improvement is not claimed.

## Implementation and evidence

- Added `http2_hpack_preserves_empty_duplicate_and_large_header_values` using
  Tokio's in-memory `duplex`, so it does not depend on loopback sockets.
- Documented that HPACK is supplied by `h2` and that the test covers the
  integration boundary rather than a project-owned codec.
- Test command: `cargo test -p espejismo-core http2_hpack_preserves_empty_duplicate_and_large_header_values`.
- Result: passed. The focused test passed (1 passed); the full
  `cargo test -p espejismo-core` suite passed, including 237 unit tests, 11
  integration tests, and 1 doctest. One pre-existing loopback listener test is
  ignored as required in the sandbox. No performance claim or measurement
  applies to this correctness-only change.
