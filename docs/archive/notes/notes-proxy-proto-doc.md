# Upstream proxy protocol documentation

## Findings and plan

`docs/deployment/EGRESS.md` already listed supported upstream proxy schemes and
TCP/UDP capabilities, but left protocol-specific target addressing, auth
negotiation, and chained-DNS policy boundaries implicit. The implementation
shows that SOCKS5 sends domain names in CONNECT requests, SOCKS4a sends its
domain extension, SOCKS4 accepts IPv4 literals only, and HTTP(S) uses HTTP/1.1
CONNECT. SOCKS5 optionally negotiates username/password; HTTP(S) uses Basic
proxy auth. UDP chaining is implemented only for SOCKS5.

The change adds these details and the security consequence of proxy-side name
resolution. Name and port rules and private IP literals are still checked
before chaining, but `deny_private_ips` cannot inspect the address a proxy
resolves for a domain. Operators relying on that control must enforce it at the
upstream proxy too. This is a documentation clarification; it does not alter
the product's single-purpose tunnel positioning or add protocols.

The reference index points to sing-box for configuration and proxy-framework
organization and shadowsocks-rust for focused Rust proxy implementation.
Following that documentation approach, the protocol matrix is paired with
operationally relevant limits and distinctions rather than a broad protocol
catalog.

## Validation

- Manually compared the scheme table and behavior notes against
  `crates/espejismo-core/src/egress.rs`,
  `crates/espejismo-server/src/relay.rs`,
  `crates/espejismo-server/src/socks5_chain.rs`, and
  `crates/espejismo-server/src/http_chain.rs`.
- Documentation-only change: no runtime behavior changed, so no benchmark or
  Rust test run was applicable. Validation result: descriptions match the
  inspected implementation; no regression introduced by code changes (none
  made).
