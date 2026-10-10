# SOCKS5 UDP relay boundaries

## Findings and approach

The UDP ingress parser already checked SOCKS reserved bytes, address truncation,
fragment sequencing, source consistency, and the 16-bit reassembly cap. Its
IPv4/IPv6 and domain header tests exercise every truncation boundary. One gap
remained: unlike the TCP SOCKS request parser, the UDP parser accepted an empty
domain or a domain containing NUL. Those values are not valid destination names
and can fail later in a less clear egress stage.

The repository reference list points to shadowsocks-rust for UDP relay and
sing-box for proxy protocol structure. The relevant transferable principle is
to validate packet framing at ingress before handing destinations to the relay;
this keeps validation local and does not alter Espejismo's TCP tunnel, proxy
identity, or egress policy.

Change `parse_udp_packet_inner` to apply the same non-empty UTF-8/no-NUL domain
rule already used by SOCKS5 TCP requests, and add a focused boundary regression
test. This has no expected throughput effect; it rejects malformed packets
before relay work and gives callers a consistent parse error.

## Validation

`cargo test -p espejismo-core ingress::socks5::tests::udp_packet_rejects_empty_and_nul_domain_names` passed (1 test). The full SOCKS5 ingress test module passed (23 tests), covering malformed domains, every IPv4/IPv6/domain address-header truncation boundary, fragment sequence gaps and target/source changes, and the wire payload cap. `cargo test -p espejismo-core` passed (207 unit tests, 1 integration config test, 8 HTTP proxy tests, and 1 doctest; 1 loopback-dependent test ignored by the repository's sandbox rule).

This is a correctness and input-validation change; no throughput benchmark applies. No performance claim is made.
