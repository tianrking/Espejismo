# Glossary

This glossary defines the terms used in Espejismo's user and protocol
documentation. For wire-level requirements, see the [protocol specification](docs/PROTOCOL.md).

| Term | Meaning in Espejismo |
| --- | --- |
| **Authenticated encrypted chaos** | A description of the observable encrypted traffic: authenticated encryption and changing or masked metadata avoid relying on a recognizable plaintext protocol marker. It is not a claim that traffic is invisible or undetectable. |
| **Client / remote** | The client is `espejismo-local`, which accepts local application traffic. The remote is `espejismo-remote`, which authenticates tunnel sessions and connects to permitted destinations. “Server” may be used informally for the remote process. |
| **Client import profile** | An `espejismo://import/...` URL that encodes selected local client settings for transfer or conversion to TOML. It includes the PSK and is secret material; base64 encoding does not encrypt it. |
| **Config profile** | A named built-in overlay selected with `--profile`, applied before command-line overrides. It is distinct from the obfuscation profile in shared transport settings. |
| **DNS takeover** | In TUN mode, an opt-in change to the host's DNS resolver settings that directs lookups to configured DNS server addresses. It is separate from Espejismo's DNS resolution behavior and can be restored during cleanup. |
| **Egress policy** | The remote-side rules that determine which destination addresses and ports a tunnel request may reach. |
| **Frame** | A unit carried by Espejismo's encrypted transport. Frame types include data, close, padding, and key update. In stealth mode, frames use a selected fixed wire size. |
| **Handshake window** | A time-based key derivation setting that limits which time slots the remote accepts for a new handshake. It bounds the useful lifetime of a captured initial packet; it is separate from session key updates. |
| **Lane** | One authenticated physical tunnel connection from the client to the remote, with its encrypted frame transport and one mux session. The client can pool lanes and assign new logical streams to them. |
| **Logical stream** | One independently opened mux channel within a lane. It carries a tunnel request and its TCP relay or UDP datagram exchange; ending a stream does not necessarily end its lane. |
| **Mux / multiplexer** | The layer that carries multiple logical streams over one lane. `yamux` is the production mode; the in-tree native mux is a beta option. |
| **Obfuscation profile** | A configuration choice for chunking and, for `stealth`, fixed-size encrypted frames and related pacing behavior. The name describes traffic shaping behavior, not protocol impersonation or guaranteed concealment. |
| **Physical connection** | The underlying network connection accepted by the remote. In the normal production path this is TCP; one physical connection corresponds to one lane/session. |
| **Probe (`--probe-server`)** | A client diagnostic that checks TCP reachability and completes the Espejismo handshake. It does not start proxy or TUN listeners. |
| **Port hopping** | Deterministic selection of a configured remote TCP port for each new physical connection based on a time window. It does not change the handshake or encrypted frame protocol. |
| **PSK (pre-shared key)** | A secret configured on both client and remote and used as input to handshake authentication and key derivation. It must be kept private and configured consistently. |
| **TUN** | A native virtual network interface used by the client to capture system IP traffic and convert supported flows into tunnel requests. It is an optional client ingress mode, not a separate remote protocol. |
| **Route takeover** | An opt-in TUN operation that installs host routes so supported system traffic uses the virtual interface while preserving a route to the remote endpoint. Espejismo records state for cleanup and recovery. |
| **Underlay** | The byte-carrying transport beneath Espejismo's handshake and encrypted frames. TCP is the default; WebSocket and HTTP/2 modes carry the same Espejismo protocol over their respective TCP connections. |
| **UDP underlay** | Experimental packet-transport primitives in the core. They are distinct from SOCKS5 UDP relay, and are not the default production tunnel transport. |
| **UDP relay** | Forwarding of SOCKS5 UDP ASSOCIATE datagrams through logical tunnel streams and the existing TCP tunnel. It does not mean the tunnel itself uses UDP. |
| **Upstream proxy** | An optional proxy used by the remote to reach an allowed destination. It is an egress chaining option and is independent of the client's local proxy listener. |
| **Wire protocol version** | The version identified during the authenticated handshake. Peers currently require an exact match; the binary release version does not negotiate compatibility. |
| **CGNAT (carrier-grade NAT)** | An ISP-managed layer of address translation outside the operator's router. A port forward on the local router cannot by itself provide inbound reachability through CGNAT. |
| **X25519 / HKDF / XChaCha20-Poly1305** | Cryptographic building blocks: X25519 establishes shared session material, HKDF derives keys, and XChaCha20-Poly1305 authenticates and encrypts frames. |

## Scope notes

- Espejismo does not impersonate TLS, HTTP, HTTP/2, QUIC, or another protocol.
- “Stealth” and “chaos” describe implementation choices, not an invisibility
  guarantee or protection against every traffic-analysis technique.
- Production tunnel transport and SOCKS5 UDP relay use TCP. Experimental UDP
  underlay components should not be confused with that relay behavior.
