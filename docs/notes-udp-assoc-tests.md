# SOCKS5 UDP ASSOCIATE boundary tests

## Findings and approach

`handle_udp_associate` keeps an association alive while UDP traffic arrives and
returns an error after the configured idle timeout. SOCKS UDP fragments are
reassembled in `SocksUdpReassembler`; this sequence already expires after the
RFC 1928 five-second minimum and rejects gaps, target changes, malformed input,
and oversized payloads. Source pinning compared only the sender IP, however, so
a fragment arriving from a different UDP port on the same host could complete
the original sender's sequence.

The references guide points to shadowsocks-rust for UDP relay codec design. The
applicable principle is to validate relay packet boundaries and origin
consistency before forwarding; this change does not add a transport or alter
the authenticated TCP tunnel or project positioning. Store the complete UDP
socket address for an in-progress fragment sequence and reject a port change.
Add deterministic tests for source-port change, fragment timeout recovery, and
completed-payload packet encoding/decoding. The expected effect is correct
rejection of cross-endpoint fragment mixing, with no throughput change.

## Changes

- Fragment reassembly now pins both source IP and source UDP port.
- SOCKS5 deployment docs state endpoint pinning and association idle timeout.
- Added in-memory tests for timeout recovery, endpoint changes, and forwarding
  packet framing; these tests do not bind sockets.

## Verification

`$HOME/.cargo/bin/cargo test -p espejismo-core ingress::socks5::tests --offline`
passed (31 tests, 0 failed). The full `$HOME/.cargo/bin/cargo test -p
espejismo-core --offline` run passed (286 unit tests, 1 documented config
example, 10 HTTP proxy tests, and 1 doctest; 1 loopback-dependent test ignored
under the sandbox rule). Coverage includes deterministic expiration recovery,
changed source IP and UDP port, ordered fragment completion, and
encoding/decoding the completed payload as the forwarded SOCKS UDP packet.
This is correctness work; no throughput benchmark applies and no performance
claim is made.
