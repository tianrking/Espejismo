# Buffer reuse and retention tests

## Findings and scope

The repository has no standalone buffer pool with explicit checkout/return
semantics. The relevant reusable-buffer safeguard is tokio-yamux's per-stream
`read_buf: Vec<BytesMut>`: after draining, it shrinks an unusually oversized
queue so a burst does not pin its high-water allocation for the stream's
lifetime. The queue is stream-owned, so this work tests the existing retention
policy instead of adding a new allocation pool without a production use case.

The policy is now a small pure predicate used by `poll_read`. Regression tests
pin its strict capacity and density thresholds, while the existing drain test
checks payload integrity, empty queue state, and capacity release. The existing
small-queue read test continues to cover retained capacity on normal traffic.

## Reference and expected benefit

`docs/research/REFERENCES.md` identifies HashiCorp yamux as the mux reference;
the current code's bounded queue retention follows the same practical buffer
lifetime concern. Espejismo's TCP/yamux protocol and positioning are unchanged.
Expected benefit is earlier detection of regressions that either retain a
large burst allocation or shrink a dense/small queue unnecessarily. This is a
correctness and memory-retention test change, with no runtime behavior change
and no throughput improvement claimed.

## Validation

- `$HOME/.cargo/bin/cargo test --offline -p tokio-yamux read_buffer_ -- --nocapture`:
  passed (2 tests). Covers the exact capacity boundary, dense versus sparse
  queue threshold, and end-to-end drain payload/capacity release.
- `$HOME/.cargo/bin/cargo test --offline -p tokio-yamux --lib`: passed (46
  tests), including `test_frame_read_more_than_one` for retaining ordinary
  queue capacity.
- The `window_update_deadlock` integration test is ignored because it requires
  loopback bind; outside the sandbox run
  `cargo test -p tokio-yamux --test window_update_deadlock -- --ignored`.
- No performance benchmark was run because this is a regression-test-only
  correctness/memory-retention change; no throughput claim is made.
- Initial test compilation found a missing test-module import, which was fixed.
  The workspace initially had no free disk space; `cargo clean -p tokio-yamux`
  released 1.1 GiB, after which both test commands completed successfully.
