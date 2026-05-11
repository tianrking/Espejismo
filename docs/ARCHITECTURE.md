# Espejismo Architecture

## Goals

Espejismo is a native Rust encrypted transport for public and untrusted
networks. The implementation keeps the protocol split into small modules so the
cryptographic handshake, replay protection, framing, padding, and application
relay can evolve independently.

## Crates

- `espejismo-core`: shared protocol library. It owns the handshake, replay
  cache, client puzzle, AEAD frame codec, padding generation, SOCKS5 parsing,
  HTTP proxy parsing, configuration/profile loading, encrypted transport
  adapter, UDP underlay primitives, update metadata checks, and adaptive frame
  writer.
- `espejismo-client`: builds `espejismo-local`, the local SOCKS5 and HTTP proxy
  ingress.
- `espejismo-server`: builds `espejismo-remote`, the authenticated remote
  egress.

## Handshake Pipeline

The first packet is variable length and authenticated:

```text
[ HMAC-SHA256 32 ][ UTC timestamp 8 ][ nonce 24 ][ X25519 public key 32 ][ protocol version 2 ][ capabilities 8 ][ puzzle nonce 8 ][ padding length 2 ][ padding 0..N ]
```

The client solves a bounded SHA-256 leading-zero puzzle over the body before it
computes the HMAC. The remote verifies the puzzle before the HMAC, checks the
timestamp skew, and records the ephemeral public key in a bounded replay cache.
The current protocol version is `1`; capability bit 0 enables TCP CONNECT and
bit 1 enables SOCKS5 UDP ASSOCIATE datagram relay.

## Frame Pipeline

After the handshake, both sides use HKDF-derived XChaCha20-Poly1305 keys. Frames
normally use length-prefixed encrypted super-frames:

```text
[ masked ciphertext length 4 ][ AEAD(frame type || payload) ]
```

The 4-byte length field is XOR-masked with a per-direction HKDF-derived
header-mask stream keyed by frame sequence. This keeps the robust length-based
framing model while removing the plaintext TLV length signal from the wire. Any
AEAD failure is fail-fast: the caller receives an error and the physical TCP
connection is discarded. The protocol does not try to resynchronize inside a
corrupted byte stream.

`spawn_frame_transport` bridges the frame codec into a Tokio `DuplexStream`.
This gives upper layers a normal `AsyncRead + AsyncWrite` object while keeping
AEAD, padding, jitter, and fail-fast behavior inside the protocol core.

When `[shared.obfuscation].profile = "stealth"`, the transport switches to
fixed-size encrypted frames:

```text
[ AEAD ciphertext exactly shared.stealth.frame_size bytes ]
```

The plaintext inside each stealth frame is `type || payload_len || payload ||
random_padding`, padded before encryption so data, close, and padding frames
share one wire size. The stealth upload pump sends a short random padding
warmup after handshake completion, then emits data or padding on a paced
schedule. Idle padding decays toward slower heartbeat-like intervals and active
payload resets the cadence.

## Stealth Handshake Wrapper

Plain mode uses the variable-length first packet described above. Stealth mode
wraps the client hello and server hello in fixed-size blocks that match the
configured stealth frame size. Each block starts with a random 24-byte nonce and
masks the hello plus random padding with an HMAC-derived XOR stream keyed by the
PSK auth key. This removes the plain-mode client/server hello size signature
without changing the underlying X25519/HMAC/puzzle handshake semantics.

## Multiplexing Pipeline

`espejismo-local` maintains a reconnecting authenticated physical TCP tunnel to
`espejismo-remote`, wraps it in the encrypted frame transport, and runs yamux
over it. Each accepted SOCKS5 or HTTP proxy connection opens a yamux logical
stream and sends an internal command preface. HTTP CONNECT is accepted directly;
absolute-form `http://` requests are rewritten to origin-form before entering
the tunnel. SOCKS5 UDP ASSOCIATE datagrams are relayed as UDP command streams.
Optional native TUN ingress creates a local virtual network interface and uses a
userspace netstack to convert captured TCP flows and UDP datagrams into the same
internal tunnel commands. The remote side does not need a separate TUN-specific
listener.

`espejismo-remote` accepts yamux streams over the same physical tunnel. Each
logical stream is handled independently: read the command preface, validate
egress policy, connect the outbound TCP destination or relay one UDP datagram,
then return traffic through the tunnel.

The current production tunnel still uses TCP as the physical underlay. The core
crate also contains UDP underlay primitives: packet codec, session id, sequence
numbers, cumulative ACKs, retransmission scheduling, and a portable congestion
controller with slow-start, additive growth, and loss backoff. Those primitives
are intentionally separated from the running TCP tunnel so future UDP socket
integration can reuse them without disturbing the stable proxy path.

The recommended production path is TCP/yamux because it is portable, simple to
deploy, and predictable under ordinary NAT, cloud firewall, and QoS devices.
SOCKS5 UDP ASSOCIATE is an application-level UDP relay carried over the same
authenticated TCP tunnel. A physical UDP underlay remains experimental reserve,
not the default reliability path.

## Configuration Pipeline

Both binaries read the same TOML document and use their relevant sections:
`shared`, `local`, and `remote`. Command-line flags override file values.
Operators can provide TOML from a path with `--config` or from a base64 string
with `--config-base64`. `--print-example-config` and
`--print-example-config-base64` generate deployable starter configs.
`--print-config-base64` converts a selected config into a one-line import
string, and `--decode-config-base64` prints that string back as TOML.
`--check-config` performs deployment diagnostics before startup.

`espejismo-local --print-client-profile` creates an `espejismo://import/...`
profile URL for client onboarding. `--import-profile` applies that profile to a
local config before startup.

`espejismo-remote` can hot-apply runtime settings through the authenticated admin
endpoint. `POST /reload` re-reads the original config source; `POST /apply`
accepts a TOML request body. New physical tunnels and newly opened logical
streams see the new users, quotas, bandwidth limits, egress policy, fallback,
and transport-shaping settings.

## Users And Egress

The remote can authenticate multiple users. Each `[[remote.users]]` entry has an
independent PSK and optional quota/bandwidth policy. Metrics are emitted both
globally and per user.

Remote egress policy supports host and port allow/block lists, private-address
denial for direct egress, and optional no-auth SOCKS5 chaining. TCP uses SOCKS5
CONNECT. UDP uses SOCKS5 UDP ASSOCIATE when `remote.egress.socks5_proxy` is set.

## Source Layout

`espejismo-core` is organized by responsibility:

- `config/`: TOML and base64 configuration loading.
- `crypto/`: authenticated first packet, X25519, HKDF, and AEAD helpers.
- `ingress/`: local protocol parsers such as SOCKS5 and HTTP proxy.
- `protocol/`: encrypted frames, puzzles, UDP underlay primitives, and replay
  protection.
- `transport/`: bridge between encrypted frames and `AsyncRead + AsyncWrite`.

## Adaptive Padding

Padding is optional and bounded. `FrameWriter` measures write latency as a
portable backpressure signal. If a write exceeds the configured threshold,
padding is disabled for a cooldown window, giving priority to real payload.

## Connection Lifecycle

Invalid or incomplete handshakes receive no application-layer response. The
remote can apply a short bounded silent delay before closing, but it does not
retain unbounded connections.

When `reject_delay_ms = 0`, invalid sockets are moved into a global bounded
silent tarpit pool. The pool has a hard capacity and time-to-live with oldest
entry eviction, so file descriptor and memory usage remain bounded. The tarpit
does not send drip bytes to unknown peers.

If HTTP fallback is enabled, HTTP-looking probes can be forwarded to a configured
upstream. Without an upstream, the built-in fallback returns a small HTTP 200
response with dynamic Date, Last-Modified, ETag, Content-Length, Connection, and
Server headers. A real upstream web server remains the recommended production
fallback because it naturally supplies a fuller fingerprint.

## Next Layer: Migration

Multiplexing is implemented with yamux. Transparent migration across a failed
physical tunnel remains a higher-level connection-manager task: it should own
reconnection, unsent-buffer tracking, stream retry policy, and user-visible
failure semantics. The current implementation fail-fasts corrupted physical
connections and lets the local process surface stream errors cleanly.
