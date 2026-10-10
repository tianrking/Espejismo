# Proxy chain end-to-end tests

## Scope and approach

The server's egress policy selects one upstream proxy. A multi-hop route is
therefore composed by an upstream SOCKS5 proxy forwarding its CONNECT tunnel to
a second SOCKS5 proxy; the server remains configured for one hop and does not
gain a new proxy-list feature. The test will exercise the real egress connector
and byte relay across two local SOCKS5 hops and a local echo target. This follows
the small, explicit transport-adapter approach used by sing-box and
shadowsocks-rust, while preserving Espejismo's existing native tunnel and simple
configuration model.

Expected benefit: detect broken CONNECT negotiation or byte forwarding at any
boundary of a composed proxy route, with no production-path overhead.

## Implementation and evidence

Added a `relay` unit test that calls the production egress connector configured
with the first SOCKS5 hop, negotiates CONNECT, forwards through a second local
hop, and checks an echoed payload. It covers SOCKS5 greeting/request handling,
upstream connection, and bidirectional payload forwarding without changing
production code or configuration.

Evidence on 2026-10-06:

- `cargo test -p espejismo-server --bin espejismo-remote --no-run`: passed;
  confirms the new test and server target compile.
- `cargo test -p espejismo-server --bin espejismo-remote relays_tcp_through_two_socks5_hops`:
  could not execute in this environment. It failed at the first
  `TcpListener::bind("127.0.0.1:0")` with `PermissionDenied` / `Operation not
  permitted`, before exercising the test path.
- `git diff --check`: passed.

No correctness pass or regression-free conclusion can be claimed until the
targeted test runs in an environment that permits loopback sockets. Expected
benefit is coverage only; there is no runtime or performance change.
