# Chunked prebuffer boundary tests

## Scope and approach

`write_all_chunked` forwards HTTP prebuffer data in fixed 128 KiB writes. The
existing regression test checked a 300 KiB payload, but did not pin down empty
input or exact buffer boundaries. Added an explicit empty-input fast return and
a boundary regression covering 0, 1, 128 KiB - 1, 128 KiB, 128 KiB + 1, and
two full buffers plus a seven-byte tail. Every case checks byte-for-byte
preservation through an in-memory duplex stream.

This is correctness coverage only; it does not change chunk size, framing,
protocol behavior, or the project's no-camouflage positioning. The empty-input
return avoids entering the chunk loop when there is nothing to write. Expected
throughput impact is neutral; no performance claim is made.

## Validation

- `cargo test -p espejismo-client`: 53 passed, 0 failed, 0 ignored.
- The new boundary test exercises empty, sub-buffer, exact-buffer,
  over-buffer, and multi-buffer-plus-tail cases. The existing large-buffer test
  remains as an additional 300 KiB preservation check.
- No loopback sockets are used by these tests.

Conclusion: all client tests pass; no regressions observed.
