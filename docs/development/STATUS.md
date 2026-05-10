# Implementation Status

## Implemented

- Native Rust workspace with separate local, remote, and shared core crates.
- SOCKS5 local proxy ingress with optional username/password authentication.
- SOCKS5 UDP ASSOCIATE relay for request/response datagrams.
- HTTP proxy ingress with optional Basic proxy authentication, CONNECT, and
  absolute-form HTTP forwarding.
- Shared TOML configuration and base64 configuration import.
- Client profile export/import through `espejismo://import/...` URLs.
- Configurable compact, pretty, or JSON logging with optional file output.
- Read-only admin HTTP endpoint with health, status, and Prometheus-style metrics.
- Server-side egress policy for host/port allow and block rules.
- Variable-length authenticated first packet.
- Protocol version and capability negotiation in the authenticated handshake.
- X25519 ephemeral key exchange and HKDF session keys.
- XChaCha20-Poly1305 encrypted frames with fail-fast authentication.
- HKDF-derived masked frame length headers.
- Stealth obfuscation profile with fixed-size encrypted frames, masked
  handshake blocks, padding warmup, paced writes, and idle cadence decay.
- SHA-256 client puzzle before HMAC verification.
- Bounded replay cache for ephemeral public keys.
- Bounded silent tarpit pool for invalid handshakes.
- HTTP-looking probe fallback to a configured upstream or dynamic built-in 200
  response.
- Adaptive padding backpressure.
- Yamux multiplexing over one encrypted physical tunnel.
- Local reconnecting tunnel manager for opening new streams after tunnel failure.
- End-to-end smoke test covering authenticated SOCKS5, authenticated HTTP proxy,
  file config, and base64 config.
- CI for Linux, macOS, and Windows.
- Release artifact workflow for Linux x86_64/i686/aarch64/armv7, macOS
  aarch64, and Windows x86_64/i686/aarch64.
- Local packaging scripts for Unix-like hosts and Windows PowerShell.
- systemd and Docker deployment starter assets.

## Not Yet Implemented

- Transparent migration of already-active yamux streams across a new physical tunnel.
- Browser extension packaging.
- WASM transport crate. The current runtime uses Tokio TCP and is native-first.
- Runtime config reload, richer multi-profile control plane, and log rotation policy.
- OS-specific TCP_INFO congestion telemetry. Current backpressure is portable write-latency based.

## Architecture Direction

Keep protocol primitives independent from application ingress. Browser/WASM
support should be a separate crate that reuses pure protocol pieces where
possible and supplies a WebSocket/WebTransport-style transport, rather than
trying to compile the Tokio TCP local/remote binaries to wasm.
