# IPv6 Deployment Documentation

## Findings and approach

The implementation binds a single configured `SocketAddr` for `remote.listen`
and creates family-specific TCP sockets when dialing resolved addresses.
Consequently, explicit IPv6 listeners and IPv6 client destinations are supported,
but one config entry does not promise separate IPv4 and IPv6 binds. SOCKS5
encodes IPv6 targets. Existing TUN documentation and the path audit establish
that global TUN route takeover remains IPv4-only, and Windows TUN DNS takeover
accepts IPv4 DNS addresses only.

Document these behaviors in `docs/deployment/IPV6.md`, link it from the general
configuration and TUN guides, and give operators a short reachability checklist.
Use bracketed IPv6 literals, distinguish proxy support from TUN routing, and
avoid claiming that an IPv6 wildcard is dual-stack on every operating system.
This keeps the existing native TCP tunnel and small operator-managed config
model described in `docs/POSITIONING.md`.

## Change and expected benefit

- Add an IPv6 deployment guide covering listener binding, DNS-based client
  dialing, SOCKS5 destinations, firewall checks, and known TUN/DNS limits.
- Link the guide from configuration and TUN documentation.
- Expected improvement: fewer address-format and firewall mistakes, with no
  runtime or throughput change (documentation-only change).

## Review and verification

- Compared the statements against `crates/espejismo-server/src/main.rs`,
  `crates/espejismo-core/src/tcp.rs`, `crates/espejismo-core/src/dns.rs`,
  `docs/deployment/TUN.md`, `docs/deployment/DNS.md`, and
  `docs/notes-ipv6-path-audit.md`.
- Experiment: reviewed the new documentation links, examples, and scope against
  those implementation and documentation sources. No code changed, so no cargo
  build or test was run. Performance measurements do not apply; expected
  throughput change is 0%.
- Conclusion: documentation accurately describes the current family-specific
  bind/connect behavior and does not imply IPv6 TUN route support.
