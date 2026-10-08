# Transfer-Encoding Boundary Tests

## Findings and change

The HTTP proxy rewrites absolute request targets but forwards body bytes
unchanged. It previously copied `Transfer-Encoding` through without checking
whether its framing conflicted with the separately parsed `Content-Length`, or
whether multiple/unsupported transfer codings appeared. That leaves the proxy
and upstream free to interpret a request boundary differently.

The ingress now accepts no transfer coding or exactly one case-insensitive
`chunked` coding. It rejects empty, repeated, combined, or unsupported codings,
and rejects any request carrying both `Transfer-Encoding` and
`Content-Length`. Valid chunked bytes and trailers remain opaque and unchanged.
This is limited to ordinary absolute-form HTTP requests; CONNECT tunnel bytes
are unaffected. The extra work is a bounded pass over the already-parsed header
lines; no throughput gain is claimed or expected.

## Verification

- `cargo test -p espejismo-core ingress::http_proxy::tests --offline` passed:
  7 tests, including accepted case-insensitive chunked framing, rejected
  ambiguous/unsupported boundaries, and preservation of declared and received
  trailers.
- `cargo test -p espejismo-core --offline` passed: 234 unit tests, 1 config
  example integration test, 8 HTTP proxy integration tests, and 1 doc test;
  1 existing loopback-bind test was ignored by the sandbox.
- Correctness change only; no throughput benchmark applies. No performance
  improvement is claimed.
