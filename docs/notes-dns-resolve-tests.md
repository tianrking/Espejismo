# DNS resolve tests

## Findings and approach

`espejismo_core::dns::resolve_socket_addrs` sent numeric IPv4 and IPv6 socket
addresses through Tokio's system resolver along with hostnames. That adds an
unnecessary resolver dependency to literal endpoints. The result was also
returned as an empty vector if a resolver produced no addresses, leaving each
caller to handle that state differently.

The change returns parsed `SocketAddr` literals directly, retains the existing
10 second bound for hostnames, and reports an empty hostname result as a
contextual error. The implementation follows the small async helper style
used by Tokio's `lookup_host` API; the references list also recommends
shadowsocks-rust as a source for lightweight Rust async IO practices. No
resolver dependency, protocol, or product behavior for hostname resolution was
changed. Numeric endpoints become independent of system DNS availability and
avoid the lookup setup cost; no latency percentage is claimed because this
correctness task was not benchmarked.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core dns::tests`: 5 passed. Covers
  pending resolver timeout and its authority context, direct IPv4 and IPv6
  literals, malformed hostname authority error context, and empty resolver
  results.
- `$HOME/.cargo/bin/cargo test -p espejismo-core`: 134 unit tests, 1 config
  integration test, and 1 doctest passed; no failures.

Conclusion: numeric endpoint resolution no longer needs the system resolver;
hostname timeout behavior is retained, and the core test suite reports no
regressions.
