# SOCKS5 UDP fragment reassembly

## Findings and scope

The production UDP relay is a length-prefixed request carried over the existing
TCP/yamux tunnel. Its request length field is 16 bits, and
`read_tunnel_request` uses `read_exact`, so transport read boundaries must not
be treated as UDP datagram boundaries. TUN's smoltcp stack has IP fragmentation
disabled; this change does not add IP fragmentation or alter the tunnel
protocol. The SOCKS5 ingress previously rejected every non-zero `FRAG` value.

RFC 1928 (https://www.rfc-editor.org/rfc/rfc1928) defines a 7-bit fragment
sequence and a high-bit final-fragment marker, with a reassembly timer of at
least five seconds. We implement an ordered queue
for the active SOCKS UDP association, discard gaps/target changes, cap the
reassembled payload at the tunnel's 65,535-byte wire limit, and expire it after
five seconds. This is bounded request reassembly at SOCKS ingress; it does not
change UDP underlay framing. The UDP underlay codec remains experimental and
has a separate MTU-sized packet limit. We retain the existing bounded-frame
approach used by shadowsocks-rust's UDP relay
(https://github.com/shadowsocks/shadowsocks-rust).

## Change and expected result

The client UDP association now feeds datagrams through `SocksUdpReassembler`
before relaying. Complete packets (FRAG=0) keep their existing path. Fragments
are joined in sequence from the same source IP, only the final marker releases a
packet, and malformed sequences are discarded. Regression tests cover
successful two-part assembly, sequence gaps, source and target changes,
wire-size overflow, and continued rejection by the standalone unfragmented
parser. Expected gain: SOCKS clients that fragment large UDP datagrams can now
use the relay; no throughput increase is claimed.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core ingress::socks5::tests`
  passed: 21 passed, 0 failed. This includes all new reassembly success,
  sequence-gap, target-change, source-IP isolation, and oversize cases.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core` passed: 197 unit
  tests, 1 ignored (requires loopback bind), 5 HTTP proxy integration tests, 1
  config example integration test, and 1 doctest; zero failures.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-client` passed: 46 tests,
  zero failures.
- No throughput benchmark applies: this is a SOCKS5 correctness and
  interoperability change, with no performance claim or wire-format change.
