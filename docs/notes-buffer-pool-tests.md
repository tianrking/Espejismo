# Buffer reuse and retention tests

## Findings and scope

The repository has no standalone buffer pool with explicit checkout/return
semantics. The closest reusable-buffer safeguard is tokio-yamux's per-stream
`read_buf: Vec<BytesMut>`: after draining, it shrinks an unusually oversized
queue so one burst does not pin its high-water allocation for the stream's
lifetime. The queue is stream-owned rather than shared, so adding a new pool
would change allocation policy without evidence or a production checkout path.

This change documents that retention behavior and adds a regression test for
draining a 128-slot queue. It checks payload integrity, empty return state, and
that excess queue capacity is reclaimed. The ordinary small-buffer path remains
covered by `test_frame_read_more_than_one`, which checks that capacity is kept
while the queue is below the shrink threshold.

## Reference and expected benefit

The yamux code in this repository already bounds retained queue capacity after
large drains. This follows the general buffer-lifetime discipline used by
networking implementations such as `hashicorp/yamux` (the dependency lineage
used here), while retaining Espejismo's existing TCP/yamux behavior. There is
no throughput claim: this is a deterministic memory-retention regression test,
not a performance optimization. Expected effect is improved detection of a
future queue-retention regression, with no runtime behavior change.

## Validation

- `cargo test -p tokio-yamux read_buffer_releases_excess_capacity_after_drain`:
  passed (1 test); this directly verifies payload delivery and excess-capacity
  release after draining.
- `cargo test -p tokio-yamux`: all 41 unit tests passed, including
  `test_frame_read_more_than_one` for the below-threshold retained-capacity
  path. The integration test `window_update_deadlock` could not execute its
  socket scenario: binding the local socket returned `PermissionDenied` from
  the sandbox before test assertions ran.
- No performance benchmark was run because this is a correctness and memory
  retention regression test, not a performance change. No throughput claim is
  made.
