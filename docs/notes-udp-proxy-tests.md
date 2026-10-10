# SOCKS5 UDP proxy boundary tests

## Findings and approach

The SOCKS5 egress UDP decoder already rejects unsupported fragmentation and
truncated address headers, while correctly allowing a zero-byte UDP payload.
It accepted a domain address whose length byte was zero, and the encoder could
emit the same malformed address for an empty target. The SOCKS5 address format
requires a non-empty domain. Following the strict structural validation used by
shadowsocks-rust's UDP relay codec ([source](https://github.com/shadowsocks/shadowsocks-rust)),
the codec now rejects empty domains at both boundaries. This only validates
the proxy hop's address framing; it does not change Espejismo's tunneled UDP
protocol or product position.

## Changes and expected effect

- Reject empty SOCKS5 UDP target domains when encoding or decoding.
- Add regression coverage for both malformed directions. Existing tests retain
  coverage for legal empty UDP payloads, fragmented datagrams, and truncated
  IPv4, IPv6, and domain headers.
- Expected effect: malformed proxy responses and invalid targets fail closed;
  no performance change is expected.

## Verification

`$HOME/.cargo/bin/cargo test -p espejismo-server`: 47 passed, 0 failed, 1
ignored (the existing loopback-only TCP proxy integration test). The new
`socks5_udp_datagram_rejects_empty_domain_addresses` case covers both rejected
empty-domain paths. The codec suite also passed checks for legal empty payloads,
fragmentation, and truncated IPv4/IPv6/domain headers. No performance change
was made or expected.
