# IPv6 Path Audit

## Findings and approach

The TCP transport uses address-family-specific sockets for resolved addresses, and
the remote TCP and admin listeners accept `SocketAddr`, so explicit IPv4 and IPv6
addresses follow the matching socket family. SOCKS5 CONNECT and UDP packet codecs
already encode/decode IPv6 address types. Direct UDP egress also chooses an
unspecified bind address matching the resolved destination family.

The audit found one broken handoff: `SocksTarget::authority()` joined every host
and port as `host:port`. A literal IPv6 target therefore became ambiguous when
the authority parser split it, potentially treating the wrong colon component as
the port. Format literal IPv6 hosts as `[host]:port`; domain and IPv4 authorities
keep their existing representation. This preserves the existing transport and
configuration model and has no measurable performance effect.

TUN auto-route remains IPv4-only across Linux, macOS, and Windows, with startup
validation requiring an IPv4 server address. Windows TUN DNS also rejects IPv6
DNS entries. These are platform feature limits, not failures of the TCP/SOCKS
dual-stack paths, and expanding OS route management is outside this audit fix.

This follows the small cross-platform transport approach visible in
[sing-box](https://github.com/SagerNet/sing-box) and
[shadowsocks-rust](https://github.com/shadowsocks/shadowsocks-rust): retain native
address families and ensure protocol address encodings survive conversion into
socket authorities. The project continues to use its native TCP tunnel without
protocol camouflage, consistent with `docs/POSITIONING.md`.

## Change and verification

- Bracket literal IPv6 hosts when converting a SOCKS target to an authority.
- Add a regression test that round-trips the formatted authority through the
  egress authority parser.
- Expected improvement: IPv6 literal SOCKS CONNECT and UDP forwarding reach the
  intended address and port; no throughput change is expected.
- Experiment: `$HOME/.cargo/bin/cargo test --workspace --offline` compiled the
  workspace and passed the client (29), core (121 plus 1 docs example), server
  (16), and tokio-yamux unit (23) tests. The final tokio-yamux integration test
  `one_way_bulk_transfer_exceeding_window` could not bind its loopback listener:
  the sandbox returned `PermissionDenied (Operation not permitted)`. Therefore
  the full workspace command did not pass in this environment; no IPv6-related
  failure appeared in the completed test suites. Rerun the workspace suite in
  an environment that permits loopback sockets before treating the hard test
  gate as satisfied.
