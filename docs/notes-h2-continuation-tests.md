# HTTP/2 CONTINUATION boundary tests

## Findings and approach

The HTTP/2 underlay in `crates/espejismo-core/src/underlay.rs` delegates frame
parsing and header-block sequencing to the `h2` crate. These in-memory raw-wire
tests pin CONTINUATION boundaries without adding a second frame parser or
changing Espejismo's cleartext prior-knowledge HTTP/2 underlay. This follows
Xray-core's transport-adapter approach: use a real HTTP/2 implementation,
without introducing protocol-specific camouflage or a parallel codec.

Expected benefit: no throughput change is intended. Coverage detects failures
to accept valid split headers; wrong-stream continuation; frames interleaved
before END_HEADERS (including PING); limits applied to a header list assembled
across frames; and incomplete blocks that should wait for continuation or
terminate cleanly when the peer disconnects.

## Implementation and evidence

- Added three Tokio in-memory tests to the existing three CONTINUATION tests in
  `crates/espejismo-core/src/underlay.rs`; no loopback sockets are used. The new
  cases check PING interleaving, max header-list enforcement across frames, and
  a partial block remaining pending until peer disconnect.
- Reference reviewed: Xray-core's transport-adapter model, as listed in
  `docs/research/REFERENCES.md`. The `h2` crate remains the protocol parser.
- Focused command: `cargo test -p espejismo-core http2_continuation -- --nocapture`.
  Result: 6 passed, including all six valid/malformed/waiting branches above.
- Full command: `cargo test -p espejismo-core`. Result: 305 unit tests passed,
  1 existing loopback-bind test ignored, 11 integration tests passed, and 1
  doctest passed. This includes all six CONTINUATION tests. No failures.
- No performance claim or measurement applies to this correctness change.
