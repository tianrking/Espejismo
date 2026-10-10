# UDP NAT Mapping Timeout and Refresh

## Findings and approach

The TUN ingress receives UDP datagrams from `netstack-smoltcp` with their local
and remote socket addresses, then relays each datagram independently through
the authenticated TCP/yamux tunnel. There is no application-owned UDP NAT
session table to expire or refresh. `udp_timeout_secs` bounds the response wait
for each relayed datagram; a later datagram on the same address pair is a new
relay task and starts a fresh wait window. This keeps the existing TCP-first
underlay and minimal operations model intact.

This follows the bounded per-datagram relay approach used by
[shadowsocks-rust](https://github.com/shadowsocks/shadowsocks-rust) at the UDP
relay boundary. An application NAT mapping cache would duplicate the userspace
netstack's socket mapping without providing a concrete benefit here, so none
was added. No throughput claim applies; this is timeout lifecycle correctness.

## Changes and expected effect

- Extract response framing and timeout handling into `read_udp_response` so the
  configured timeout behavior is isolated and testable.
- Document that `udp_timeout_secs` applies to one datagram and later datagrams
  receive a fresh response window.
- The expected effect is bounded wait time for unanswered responses and no
  timeout inheritance by later packets in the same local/remote address pair;
  there is no throughput change expected.

## Verification

- `cargo test -p espejismo-client tun::tests::udp_response_timeout_expires_and_next_datagram_gets_a_fresh_window`
  passed. It verifies timeout expiry with no response, followed by successful
  receipt of a delayed response in the next window.
- `cargo test -p espejismo-client` passed: 45 tests, 0 failures.
- `cargo fmt --check` could not pass repository-wide because it reports
  pre-existing formatting differences in unrelated files. The changed Rust
  file was formatted directly with `rustfmt --edition 2021`.
