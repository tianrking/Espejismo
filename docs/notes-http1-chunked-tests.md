# HTTP/1.1 chunked request boundary tests

## Findings and approach

`ingress::http_proxy` validates that `Transfer-Encoding` is a single `chunked`
coding and rejects ambiguous `Content-Length` combinations. It deliberately
forwards request bodies without decoding them. Chunk sizes, extensions, the
terminal zero chunk, and trailer fields therefore belong to the upstream HTTP
parser, which observes the complete streamed request. Partial validation of
the bytes read alongside the headers would incorrectly reject valid bodies
split across reads.

The referenced project list in `docs/research/REFERENCES.md` focuses on tunnel,
multiplexing, and proxy architecture and provides no HTTP/1 chunk decoder to
reuse here. The change keeps the existing transparent proxy behavior and adds
an explicit comment and regression coverage rather than introducing a parser
or changing Espejismo's protocol role.

Added an in-memory duplex regression exercising a chunk size larger than
`u64`, a body without its terminal chunk, a valid chunk extension, and an
invalid trailer field. Each case confirms that already-read bytes are
preserved byte-for-byte for upstream validation. This is correctness coverage;
expected performance change is neutral and no throughput claim is made.

## Validation

- `$HOME/.cargo/bin/cargo test -p espejismo-core ingress::http_proxy::tests --offline`:
  11 passed, 0 failed, 0 ignored; includes all chunked edge cases and the
  existing transfer-encoding ambiguity and trailer-preservation tests.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline`: started the full
  core suite (322 unit tests); output showed the loopback listener test ignored
  as required, but the command did not return a completion summary in this
  environment and was interrupted. Treat the focused HTTP proxy test result as
  the completed validation evidence.
- `$HOME/.cargo/bin/cargo fmt --all -- --check` reports pre-existing formatting
  differences in `crates/espejismo-client/src/handler.rs`; the edited HTTP
  proxy source was formatted directly with `rustfmt`.
- Tests use `tokio::io::duplex`; no loopback sockets are created.

Conclusion: focused HTTP proxy tests pass. The test verifies transparent
forwarding of the requested boundary cases; malformed chunk rejection remains
the responsibility of the upstream HTTP server.
