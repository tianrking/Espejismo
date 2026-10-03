# Handshake latency analysis

## Scope and findings

This pass rechecked the cold-start path for round 021. The relevant code is
`connect_tcp_stream` in `crates/espejismo-core/src/tcp.rs` and
`connect_handshake` / `accept_handshake` in
`crates/espejismo-core/src/crypto/mod.rs`; the wire contract is in
`docs/PROTOCOL.md`.

For a new direct TCP connection, the client first completes the TCP open. It
then writes one complete client hello and waits for the server hello. The
server reads and validates that hello, then writes its reply. There is no
separate challenge or acknowledgement round trip in the Espejismo handshake.
The cold-start network floor is therefore approximately **2 RTTs**: one TCP
connect RTT plus one authenticated application-handshake RTT. Crypto and
processing time are additional, local costs. WebSocket and HTTP/2 underlays
have their own setup exchanges before this shared handshake and are outside
that 2-RTT direct-TCP estimate.

The production mux is yamux. Its upstream protocol specifies no connection
setup handshake, and a newly opened stream can send data after its SYN without
waiting for the SYN acknowledgement. This can avoid an extra wait when opening
logical streams, but it cannot remove the physical connection's Espejismo
authentication RTT because mux traffic follows the authenticated handshake.
Reference: [hashicorp/yamux protocol specification](https://github.com/hashicorp/yamux/blob/master/spec.md).

The Shadowsocks TCP design also demonstrates sending initial encrypted data
with its first stream bytes, but that protocol does not provide Espejismo's
mutual authenticated session establishment, ephemeral X25519 exchange, and
replay checks. It is not a drop-in optimization for this handshake.
Reference: [Shadowsocks protocol](https://github.com/shadowsocks/shadowsocks-org/wiki/Protocol).

## Options and decision

TCP Fast Open could place hello bytes on the SYN and potentially overlap the
application hello with connection establishment. It depends on client/server
OS support, socket configuration, middlebox behavior, and a deliberate policy
for retransmitted early data. Sending application data before the current
server-authenticated response also needs a protocol design that preserves
replay protection, authentication ordering, and compatibility. Neither option
is a safe local implementation tweak, so this pass makes **no protocol or code
change**. This keeps the project's authenticated-chaos protocol and small
operational model intact.

## Expected and measured impact

No runtime behavior changed, so this pass claims **0% measured latency
improvement**. The current direct-TCP estimate remains 2 RTTs before local
processing costs; no before/after performance experiment is applicable to this
documentation-only analysis. A future TFO or early-data proposal should be
separately scoped with explicit replay/compatibility semantics and measured on
real TCP paths with TFO enabled and disabled. A loopback benchmark would not
establish cross-ocean RTT savings.

## Reference review

The project's reference list points to yamux for mux behavior and
shadowsocks-rust for Rust async I/O and AEAD framing. The yamux protocol's
no-extra-setup behavior is directly relevant and already avoids an avoidable
logical-stream RTT. Shadowsocks' first-stream-byte pattern is useful context,
but its security and protocol guarantees differ. Hysteria2 and quic-go focus
on QUIC transport and congestion behavior; migrating to QUIC would violate the
documented TCP/yamux product direction and is not relevant to shaving an
application-handshake RTT here. No dependency or product-positioning change is
proposed.
