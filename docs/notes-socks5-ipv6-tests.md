# SOCKS5 IPv6 address boundary tests

## Findings and approach

The SOCKS5 CONNECT parser and UDP codec already accept IPv6 wire addresses,
format IPv6 authorities with brackets, and keep address-family selection in the
SOCKS5 ATYP field. Existing checks covered ordinary `2001:db8::1` round trips,
but not unspecified/loopback literals, mapped IPv4-in-IPv6 bytes, port extremes,
or the fallback from non-IP text to a domain ATYP. That fallback could encode
an empty or NUL-containing host as a malformed UDP domain, even though the
parser rejects those values.

Following the framing-first validation approach used by shadowsocks-rust's UDP
relay and sing-box's proxy address handling (see `docs/research/REFERENCES.md`),
keep the change at the SOCKS5 boundary: reject invalid domain fallback values,
and test IPv6 wire format, canonical rendering, bracketed authority, mapped
IPv6 family preservation, and port/address-length boundaries. This does not
alter the tunnel protocol or proxy positioning. Expected performance change is
zero; the added checks only reject malformed input before forwarding.

## Changes and verification

- `build_udp_packet` now rejects empty and NUL-containing fallback domains,
  matching the ingress parser's existing validity rule.
- Added duplex-based SOCKS5 CONNECT tests for `::`, `::1`, IPv4-mapped IPv6,
  port 0, and port 65535; these create no local sockets.
- Added UDP codec tests verifying IPv4-mapped IPv6 retains ATYP IPv6, plus
  empty/NUL and 255/256-byte domain boundaries.

Validation:

- `cargo fmt --all` completed successfully.
- `cargo test --offline -p espejismo-core` passed: 293 unit tests, 1 config
  example integration test, 10 HTTP proxy integration tests, and 1 doctest; 1
  loopback-bind test remained ignored under the sandbox rule.
- The new regression coverage passed for IPv6 unspecified/loopback/mapped
  addresses, ports 0/65535, UDP ATYP preservation, and domain length/content
  boundaries. No local sockets were created by these tests.

No throughput benchmark applies to this correctness change. No performance
claim is made; expected throughput impact is 0%.
