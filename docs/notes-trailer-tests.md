# HTTP Trailer Boundary Tests

## Change and rationale

The HTTP proxy forwards chunked request bodies as opaque bytes after rewriting
the absolute request target. Added a regression test for a request whose headers
declare `Trailer` fields and whose already-buffered chunked body ends with
multiple trailer fields. The test verifies the declaration and the complete
chunk terminator/trailer section survive unchanged, and that this request does
not enter the fixed `Content-Length` path. Added a source comment to document
why the buffered bytes are intentionally not parsed or rewritten.

This preserves the existing small proxy model and requires no additional
runtime parsing or allocations. Expected throughput change: 0%; the change is
test coverage and a clarifying comment only.

## Verification

- `cargo test -p espejismo-core ingress::http_proxy::tests::chunked_request_preserves_declared_and_received_trailers`
  passed (1 test), covering the declaration, chunk terminator and multiple
  trailer fields in bytes already read with the headers.
- `cargo test -p espejismo-core` passed: 231 unit tests, 8 integration tests,
  1 doc test; 1 existing loopback-bind test was ignored by the sandbox.
- No behavior or performance change is expected; no throughput benchmark was
  applicable to this test-only change.
