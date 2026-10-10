# SOCKS5 UDP boundary tests

## Findings and approach

The existing ingress tests cover UDP ASSOCIATE parsing/replies, malformed UDP
headers, ordered SOCKS UDP fragments, peer/target consistency, and reassembly
size limits. Reviewing the UDP relay guidance in `docs/research/REFERENCES.md`
and shadowsocks-rust's UDP relay approach reinforced validating and containing
datagram state at the ingress boundary; this preserves Espejismo's existing
proxy behavior and does not change its tunnel identity or positioning.

One state boundary was missing: `SocksUdpReassembler::push` returned a parse
error for a malformed datagram without clearing an already active fragmented
datagram. Later fragments could then complete across that invalid input. The
reassembler now resets its accumulated target, payload, peer, and sequence
state whenever packet parsing fails. A regression test starts a fragmented
sequence, injects a truncated packet, and verifies that the old final fragment
cannot complete it. The SOCKS5 deployment guide documents the reset behavior.

Expected benefit is correctness and bounded state lifetime after malformed
input; there is no throughput claim or expected measurable performance change.

## Validation

`$HOME/.cargo/bin/cargo test -p espejismo-core ingress::socks5::tests::socks_udp_reassembler_discards_sequence_after_malformed_datagram`
passed (1 test). The full SOCKS5 ingress module passed (27 tests), including
UDP ASSOCIATE request/reply address forms, parser header truncation, fragment
ordering/source/target/reset and payload limits. `$HOME/.cargo/bin/cargo test
-p espejismo-core` passed (254 unit tests, 1 config example integration test,
10 HTTP proxy integration tests, and 1 doctest); 1 existing loopback-bind test
was ignored per the sandbox rule. No throughput benchmark applies to this
correctness-only change.
