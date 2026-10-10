# HTTP CONNECT authentication failure test

## Findings and approach

`accept_http_proxy_with_auth` checks configured Basic credentials before it
handles CONNECT and writes `200 Connection Established`. A failed check writes a
`407 Proxy Authentication Required` response and returns an error. The existing
HTTP proxy integration test covered bad credentials on an absolute-form GET, but
did not lock down the CONNECT boundary where a mistaken success response would
start a tunnel.

The public-ingress Tokio duplex tests follow the protocol-boundary testing style
used by sing-box and shadowsocks-rust: they assert the actual bytes exchanged,
without changing the project's native tunnel or non-camouflage positioning.

## Change and expected benefit

Added a regression test sending CONNECT with incorrect Basic credentials and
extra bytes after the headers. It checks that the parser returns an
authentication error, responds with 407, does not return a 200 status, and does
not forward the extra bytes as a successful tunnel response. No production
logic change or throughput gain is expected; this makes the failed-auth CONNECT
contract explicit and guards against accidental tunnel establishment.

## Experiment and result

- `cargo test -p espejismo-core --offline --test http_proxy`: 5 passed, 0 failed.
  Covers successful CONNECT with early tunnel bytes, failed-auth GET and
  CONNECT, absolute-form forwarding with credentials stripped, and rejection of
  origin-form requests.
- `cargo test -p espejismo-core --offline -q`: 183 unit tests, 1 config example
  integration test, 5 HTTP proxy integration tests, and 1 doctest passed; 0
  failed.

Correctness result: no regression; the CONNECT authentication-failure path
returns 407 and does not establish a tunnel.
