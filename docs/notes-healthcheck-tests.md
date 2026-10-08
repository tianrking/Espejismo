# Health Check Boundary Tests

## Change and rationale

The admin listener's public `GET /healthz` route previously passed through
`Content-Length` parsing and request-body reads before dispatch. A liveness
probe with an invalid, oversized, or incomplete declared body could therefore
fail or wait for the body timeout despite the process being able to answer.
The handler now returns its fixed `200` response immediately after recognizing
the exact public probe. Other routes retain their existing authorization and
body handling. The deployment guide documents the body-independent behavior.

Expected improvement: health probes no longer depend on request-body validity
or delivery; this removes up to the 15-second body wait on this path. There is
no throughput claim or protocol behavior change for other admin routes.

## Verification

- `cargo test -p espejismo-core admin::tests::health_probe_ignores_invalid_or_unfinished_request_body`
  covers invalid length, declared size over the admin body limit, and a declared
  body that is never sent. The expected response is `200` with `ok\n` in each
  case. The duplex stream test does not bind a socket.

Run result: `$HOME/.cargo/bin/cargo test -p espejismo-core --lib` passed
227 tests, with 1 pre-existing loopback-bind test ignored and no failures. The
focused regression test also passed independently (1 passed). The first full
package invocation did not return a final status in the runner; the library
suite was rerun under a 120-second process timeout and completed successfully
in 56.49 seconds. No performance claim applies to this correctness change.
