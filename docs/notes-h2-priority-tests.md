# HTTP/2 PRIORITY boundary tests

## Findings and approach

`espejismo-core` uses the `h2` crate for HTTP/2 framing and connection state;
the adapter does not maintain a separate priority tree. Existing in-memory
tests covered the five-byte payload length, stream zero, and self-dependency,
but did not pin the legal weight endpoints or the exclusive dependency bit.
The `h2` frame decoder is the relevant implementation boundary here, while its
public server API does not expose the resulting tree for assertions.

Following the small, state-machine-oriented framing tests used in mature
protocol implementations (and the repository's reference guidance in
`docs/research/REFERENCES.md`), the added regression sends legal wire PRIORITY
frames through the actual `h2` server decoder and then a valid request. It
covers weight 1 and 256, root and exclusive dependency encodings, and confirms
that subsequent request processing still succeeds. This intentionally tests
wire acceptance and connection usability; it does not claim to inspect
internal scheduling-tree state. All bytes travel over Tokio's in-memory
`duplex`, so no loopback socket or positioning change is involved.

## Expected impact

This is correctness coverage only; there is no runtime or throughput change,
so no performance gain is claimed. It should catch regressions in legal
PRIORITY boundary decoding and connection handling.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core http2_priority --offline`:
  5 passed, including the new legal dependency/weight boundary sequence.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline`: 284 unit tests,
  1 ignored loopback test, 11 integration tests, and 1 doctest passed; no
  failures. The ignored test is the existing loopback-bind listener test.
- No performance benchmark was run because this is a test-only correctness
  change and does not alter runtime behavior.
