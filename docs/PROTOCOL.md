# Espejismo Protocol Specification

This document describes the Espejismo wire protocol for the `v0.1.x` line
through `v0.1.3`. It is written as an implementation contract for compatible
clients, servers, test fixtures, and future transports.

## Requirements Language

The words MUST, MUST NOT, REQUIRED, SHOULD, SHOULD NOT, RECOMMENDED, MAY, and
OPTIONAL are used with their RFC 2119 meanings.

## Design Goals

Espejismo is an authenticated encrypted byte-stream tunnel. It does not attempt
to impersonate TLS, HTTP, HTTP/2, QUIC, or any other named application protocol.
The protocol instead aims to avoid stable cleartext markers while keeping
bounded parsing, authenticated state transitions, and predictable failure
behavior.

The guiding principle is no impersonation and low-feature transport behavior:
Espejismo does not borrow another protocol's fingerprint, certificate story,
ALPN, headers, magic bytes, or error vocabulary. It presents authenticated
encrypted chaos instead. A passive observer should not see stable plaintext TLV
markers, fixed handshake offsets, fixed frame-length metadata, or reliable
application-layer rejection messages.

This is not a claim of invisibility. It is a refusal to depend on camouflage.
The protocol favors rotating keys, masked metadata, variable or fixed-size
encrypted blocks, optional padding, fail-closed authentication, silent rejection,
and bounded public-side resource use.

The default production underlay is TCP. When `[shared.underlay].mode =
"websocket"`, peers first complete a standard HTTP/1.1 WebSocket Upgrade on the
configured path, then carry the same Espejismo handshake and encrypted frame
bytes inside WebSocket binary frames. When `[shared.underlay].mode = "http2"`,
peers use cleartext HTTP/2 prior knowledge and a POST stream on the configured
path. Multiplexing and transport adapters sit above the encrypted frame
transport so underlays reuse the same handshake, frame codec, request prefaces,
and policy layer.

When `[shared.port_hopping].enabled = true`, the client deterministically
selects a configured remote port from the current time window before opening a
new physical underlay connection. This does not change any handshake, frame, or
mux bytes.

## Numeric Encoding

Unless otherwise stated:

- Multi-byte integers are encoded in network byte order.
- Protocol string fields use UTF-8.
- Length-prefixed variable payloads MUST be checked against the configured
  maximum before allocation.

## Protocol Version And Capabilities

The current protocol version is `1`.

Handshake capabilities are authenticated as part of the client hello and server
reply. Current capability bits are:

- Bit 0: TCP CONNECT request support.
- Bit 1: SOCKS5 UDP ASSOCIATE datagram relay support.
- Bit 8: `yamux` logical stream mux support.
- Bit 9: Espejismo native mux support.

Both peers MUST advertise the configured mux mode. A mux capability mismatch
MUST fail during the authenticated handshake with an explicit capability error.

## Plain Handshake

Plain mode starts with a variable-length masked client envelope:

```text
[ random nonce 24 ][ masked payload length 4 ][ masked payload ][ random tail padding ]
```

The payload length and payload are XOR-masked with HMAC-derived streams keyed by
the handshake authentication key. When `shared.handshake_window.enabled = true`,
that key is derived from the PSK and the current time slot:

```text
slot = floor(unix_time / shared.handshake_window.step_secs)
handshake_auth_key = HKDF(PSK, "espejismo v1 handshake-window-auth-key:" || slot)
```

The client sends with the current slot. The server tries the current slot plus
the configured previous/future slot tolerance before treating the first packet
as invalid. This means an active probe that replays a recorded first packet
after the accepted window expires does not decrypt as a valid hello and receives
no Espejismo application response. The random nonce makes every first packet
non-static, and the mask moves fixed offsets out of the clear wire image while
preserving a bounded async parser.

The masked payload contains:

```text
[ HMAC-SHA256 32 ]
[ UTC timestamp ]
[ client nonce ]
[ X25519 client ephemeral public key ]
[ protocol version ]
[ capabilities ]
[ puzzle nonce ]
[ padding length ]
[ padding ]
```

The client MUST solve the configured bounded SHA-256 leading-zero puzzle over
the handshake body before computing the HMAC. The server MUST verify the puzzle
before verifying the HMAC so unauthenticated work remains bounded and tunable.

The server MUST reject a client hello when:

- The envelope is shorter than the minimum fixed body.
- The masked length exceeds configured bounds.
- The padding length exceeds `remote.max_handshake_padding`.
- The puzzle is invalid.
- No configured user/window key authenticates the HMAC.
- More than one configured user/window candidate could match the masked hello
  shape.
- The protocol version is unsupported.
- Required capabilities are absent.
- The timestamp is outside `shared.clock_skew_secs`.
- The authenticated outer first client packet digest was already seen inside
  the replay window.
- The client ephemeral public key was already seen inside the replay window.

On success, both peers derive session keys from the X25519 shared secret and PSK
context with HKDF.

## Stealth Handshake Wrapper

When `[shared.obfuscation].profile = "stealth"`, the plain handshake body is
wrapped in fixed-size blocks whose size is `shared.stealth.frame_size`:

```text
[ random nonce 24 ][ masked hello plus random padding to fixed block size ]
```

The wrapper does not change the X25519, HMAC, timestamp, puzzle, replay, or
capability semantics. It only changes the outer wire image from a
variable-length envelope to fixed-size masked blocks.

The configured stealth frame size MUST be large enough to hold the fixed
handshake body plus the nonce, length metadata, authentication data, and minimum
padding. Implementations MUST reject too-small stealth frame sizes at config
check or startup.

## Frame Transport

After handshake, both directions use HKDF-derived XChaCha20-Poly1305 keys. Plain
transport uses encrypted super-frames:

```text
[ masked ciphertext length 4 ][ AEAD(frame type || payload) ]
```

The 4-byte ciphertext length is XOR-masked with a per-direction HKDF-derived
header-mask stream keyed by the frame sequence number.

Frame types are encrypted. Current frame semantics are:

- DATA: carries tunnel or mux bytes.
- CLOSE: marks clean sender-side stream close intent at frame layer.
- PADDING: carries no application data.
- KEY_UPDATE: authenticates the transition to the next traffic secret.
- TARGET: reserved for legacy/compat framing paths; not used by the current
  request-preface tunnel flow.

An AEAD authentication failure MUST fail the physical connection immediately.
Implementations MUST NOT scan for a new frame boundary or attempt to
resynchronize inside a corrupted byte stream.

## Key Update

Every `shared.key_update_frames` transmitted frames, the sender emits an
encrypted KEY_UPDATE control frame under the current traffic key. After the
frame authenticates, the receiver derives the next traffic secret and header
mask key from the previous traffic secret with HKDF.

Key updates are per direction. A peer MUST keep sequence and key-update state
separate for read and write directions.

## Stealth Frame Mode

In stealth mode, encrypted frames are fixed-size blocks:

```text
[ AEAD ciphertext exactly selected_stealth_frame_size bytes ]
```

The encrypted plaintext contains:

```text
[ frame type ][ payload length ][ payload ][ random padding ]
```

DATA, CLOSE, PADDING, and KEY_UPDATE frames share the same outer size. The
writer MAY send a short random padding warmup after handshake completion. During
idle periods, the writer SHOULD decay toward a slower heartbeat-like cadence;
active payload SHOULD reset the cadence.

`shared.stealth_shaper` optionally changes the idle padding budget and timing
model. It does not alter the encrypted frame format and does not rate-limit
real DATA payloads.

`selected_stealth_frame_size` is chosen per authenticated session. If
`shared.stealth.frame_size_candidates` is empty, the transport uses
`shared.stealth.frame_size`. Otherwise both peers deterministically select one
candidate from `shared.stealth.frame_size_candidates` using session key
material. This keeps fixed-size protection while avoiding a single global frame
size signature across all deployments.

## Tunnel Requests

Each accepted local SOCKS5, HTTP proxy, or TUN flow opens a logical mux stream.
The first bytes on the logical stream are an internal tunnel request.

TCP CONNECT:

```text
[ command TCP_CONNECT ][ priority ][ authority length ][ authority bytes ]
```

UDP DATAGRAM:

```text
[ command UDP_DATAGRAM ][ priority ][ authority length ][ authority bytes ][ payload length ][ payload ]
```

`authority` is `host:port`. The remote MUST validate the authority and resolved
addresses against egress policy before dialing or sending.

## Multiplexing

The encrypted frame transport is adapted into an `AsyncRead + AsyncWrite`
stream. A configured mux implementation then carries logical streams over that
physical encrypted tunnel.

The production mux mode is `yamux`. The native mux mode is an in-tree beta with
OPEN, DATA, WINDOW_UPDATE, FIN, RST, PING, and GOAWAY frames. Native mux streams
MUST observe configured stream limits, bounded queues, byte-window flow control,
and graceful drain semantics.

Mux mode is part of the authenticated handshake capabilities. Peers MUST NOT
silently fall back to another mux mode after handshake.

## UDP Relay

SOCKS5 UDP ASSOCIATE is carried as application-level UDP DATAGRAM requests over
the authenticated mux tunnel. The current production path does not use a UDP
physical underlay.

The core UDP underlay packet codec and reliability/congestion primitives are
reserved for future transport integration. Implementations MUST treat that
underlay as experimental unless explicitly enabled by a future protocol version
or capability bit.

## Error Handling

Invalid or incomplete handshakes MUST NOT receive Espejismo application data.
The remote MAY silently delay closure or place the socket in a bounded silent
tarpit. Any tarpit MUST have a hard capacity and time-to-live.

The replay cache covers both the authenticated outer first client packet digest
and the X25519 client ephemeral public key. The first-packet digest is computed
over the bytes observed on the wire for the initial client handshake envelope
or stealth block. This makes exact active-probe replays inside the accepted
handshake time window fail before the server sends a response, while preserving
the public-key replay check as a second guard.

HTTP-looking probes MAY be routed to a configured fallback upstream or a built-in
HTTP response when probe fallback is enabled. Fallback behavior MUST remain
outside authenticated tunnel state.

After authentication, malformed encrypted frames, failed AEAD checks, invalid
key updates, unsupported tunnel commands, mux errors, and egress policy failures
MUST close or fail the affected stream. AEAD failure MUST close the whole
physical tunnel.

## Resource Bounds

Implementations MUST bound:

- Handshake read time.
- Handshake envelope and padding length.
- Replay-cache size by time window.
- Public-side tarpit capacity and hold duration.
- Physical connection count.
- Logical stream count.
- First request read time for each logical stream.
- Frame payload size.
- Native mux pending queues and per-stream buffers.

## Security Notes

The PSK is authentication material and MUST be random and at least 16 bytes.
Operators SHOULD use longer high-entropy secrets, especially for multi-user
remote configurations.

Traffic shaping and stealth mode can reduce stable framing signals, but they do
not make a connection invisible. Endpoint IPs, timing, total byte volume, route
changes, uptime, and deployment mistakes can remain observable.
