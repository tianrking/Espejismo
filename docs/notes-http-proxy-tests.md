# HTTP proxy protocol tests

## Findings and approach

`espejismo-core::ingress::http_proxy` already parses CONNECT and absolute-form
HTTP requests, emits `200 Connection Established` for CONNECT, strips proxy
credentials before forwarding regular requests, and emits `407` when configured
Basic authentication fails. Existing inline tests covered basic parsing but did
not exercise the wire responses or early bytes arriving in the same read as the
headers. The client handler forwards the returned prebuffer into the tunnel, so
preserving those bytes is an important ingress contract.

Following the small protocol-boundary tests used by sing-box and
shadowsocks-rust, the new integration tests use Tokio duplex streams and the
public ingress API. This keeps the tests deterministic and validates actual
request/response bytes without changing the tunnel architecture or project
positioning.

## Change and expected benefit

Added `crates/espejismo-core/tests/http_proxy.rs` with four wire-level cases:

- CONNECT returns 200 and keeps bytes received after the header as tunnel data.
- An absolute-form POST becomes origin-form, retains content length and initial
  body bytes, and removes `Proxy-Authorization` before forwarding.
- Invalid configured Basic credentials return HTTP 407 and an authentication
  error.
- An origin-form target is rejected with an error and no success response.

Expected benefit: catch regressions in the supported HTTP proxy contract,
including response status codes and request/body forwarding boundaries. No
throughput improvement is expected or claimed; this is correctness coverage.

## Experiment and result

Command: `$HOME/.cargo/bin/cargo test -p espejismo-core --offline`

Result: passed. 148 unit tests, 1 config-example integration test, 4 HTTP proxy
integration tests, and 1 doctest passed; 0 failed. This confirms no regression
in the core crate test suite. The four HTTP cases cover the branches listed
above, including both accepted and rejected requests and the 200/407 responses.
