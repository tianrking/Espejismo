# Espejismo Architecture

## Goals

Espejismo is a native Rust encrypted transport for public and untrusted
networks. The implementation keeps the protocol split into small modules so the
cryptographic handshake, replay protection, framing, padding, and application
relay can evolve independently.

## Crates

- `espejismo-core`: shared protocol library. It owns the handshake, replay
  cache, client puzzle, AEAD frame codec, padding generation, SOCKS5 parsing,
  encrypted transport adapter, and adaptive frame writer.
- `espejismo-client`: builds `espejismo-local`, the local SOCKS5 ingress.
- `espejismo-server`: builds `espejismo-remote`, the authenticated remote
  egress.

## Handshake Pipeline

The first packet is variable length and authenticated:

```text
[ HMAC-SHA256 32 ][ UTC timestamp 8 ][ nonce 24 ][ X25519 public key 32 ][ puzzle nonce 8 ][ padding length 2 ][ padding 0..N ]
```

The client solves a bounded SHA-256 leading-zero puzzle over the body before it
computes the HMAC. The remote verifies the puzzle before the HMAC, checks the
timestamp skew, and records the ephemeral public key in a bounded replay cache.

## Frame Pipeline

After the handshake, both sides use HKDF-derived XChaCha20-Poly1305 keys. Frames
are length-prefixed encrypted super-frames:

```text
[ ciphertext length 4 ][ AEAD(frame type || payload) ]
```

Any AEAD failure is fail-fast: the caller receives an error and the physical TCP
connection is discarded. The protocol does not try to resynchronize inside a
corrupted byte stream.

`spawn_frame_transport` bridges the frame codec into a Tokio `DuplexStream`.
This gives upper layers a normal `AsyncRead + AsyncWrite` object while keeping
AEAD, padding, jitter, and fail-fast behavior inside the protocol core.

## Multiplexing Pipeline

`espejismo-local` creates one authenticated physical TCP tunnel to
`espejismo-remote`, wraps it in the encrypted frame transport, and runs yamux
over it. Each accepted SOCKS5 connection opens a yamux logical stream and sends
the target authority as the stream preface.

`espejismo-remote` accepts yamux streams over the same physical tunnel. Each
logical stream is handled independently: read target authority, connect the
outbound TCP destination, then relay bidirectionally.

## Adaptive Padding

Padding is optional and bounded. `FrameWriter` measures write latency as a
portable backpressure signal. If a write exceeds the configured threshold,
padding is disabled for a cooldown window, giving priority to real payload.

## Connection Lifecycle

Invalid or incomplete handshakes receive no application-layer response. The
remote can apply a short bounded silent delay before closing, but it does not
retain unbounded connections.

## Next Layer: Migration

Multiplexing is implemented with yamux. Transparent migration across a failed
physical tunnel remains a higher-level connection-manager task: it should own
reconnection, unsent-buffer tracking, stream retry policy, and user-visible
failure semantics. The current implementation fail-fasts corrupted physical
connections and lets the local process surface stream errors cleanly.
