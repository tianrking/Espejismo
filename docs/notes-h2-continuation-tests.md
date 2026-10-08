# HTTP/2 CONTINUATION boundary tests

## Findings and approach

The HTTP/2 underlay in `crates/espejismo-core/src/underlay.rs` delegates frame
parsing and header-block sequencing to the `h2` crate. Existing raw-frame tests
cover PRIORITY boundaries, while HTTP/2 header handling is otherwise exercised
through the crate's client/server APIs. Following the same in-memory wire-test
approach, this change pins CONTINUATION behavior without adding a second frame
parser or changing Espejismo's cleartext prior-knowledge HTTP/2 underlay. This
matches Xray-core's transport-adapter model: use a real HTTP/2 implementation
rather than introducing protocol-specific camouflage or a parallel codec.

Expected benefit: no throughput change is intended. Coverage now detects
regressions in accepting a valid split header block, rejecting continuation on
the wrong stream, and rejecting an interleaved frame before END_HEADERS.

## Implementation and evidence

- Added raw-frame construction helpers and three Tokio in-memory tests in
  `crates/espejismo-core/src/underlay.rs`; no loopback sockets are used.
- Focused command: `cargo test -p espejismo-core http2_continuation -- --nocapture`.
  Result: 3 passed, covering successful split-header decode, mismatched stream
  ID rejection, and missing END_HEADERS sequencing rejection.
- Full command: `cargo test -p espejismo-core` ran 245 unit tests, including all
  three new tests; the loopback listener test remains ignored by its existing
  `requires loopback bind` annotation. A quiet full run reported a failure in
  the timing-sensitive `transport::tests::idle_copy_bidirectional_refreshes_timeout_on_traffic`;
  its isolated rerun passed. A subsequent quiet full run passed that case and
  proceeded beyond it, but the execution window ended before a final suite
  summary was returned. Thus the focused tests and isolated rerun are confirmed
  passing, while a clean full-suite completion is not confirmed in this
  environment. No performance claim or measurement applies to this correctness
  change.
