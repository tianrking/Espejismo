# Implementation Status

## Implemented

- Native Rust workspace with separate local, remote, and shared core crates.
- SOCKS5 local proxy ingress with optional username/password authentication.
- HTTP proxy ingress with optional Basic proxy authentication, CONNECT, and
  absolute-form HTTP forwarding.
- Shared TOML configuration and base64 configuration import.
- Configurable compact, pretty, or JSON logging with optional file output.
- Variable-length authenticated first packet.
- X25519 ephemeral key exchange and HKDF session keys.
- XChaCha20-Poly1305 encrypted frames with fail-fast authentication.
- HKDF-derived masked frame length headers.
- SHA-256 client puzzle before HMAC verification.
- Bounded replay cache for ephemeral public keys.
- Bounded silent tarpit pool for invalid handshakes.
- Adaptive padding backpressure.
- Yamux multiplexing over one encrypted physical tunnel.
- End-to-end smoke test covering authenticated SOCKS5, authenticated HTTP proxy,
  file config, and base64 config.
- CI for Linux, macOS, and Windows.
- Release artifact workflow for Linux x86_64, macOS x86_64, macOS aarch64, and
  Windows x86_64.
- Local packaging scripts for Unix-like hosts and Windows PowerShell.
- systemd and Docker deployment starter assets.

## Not Yet Implemented

- UDP ASSOCIATE support.
- Transparent migration of live yamux streams across a new physical tunnel.
- Browser extension packaging.
- WASM transport crate. The current runtime uses Tokio TCP and is native-first.
- Linux aarch64 release artifact automation.
- Admin API, live metrics endpoint, log rotation policy, and runtime config reload.
- OS-specific TCP_INFO congestion telemetry. Current backpressure is portable write-latency based.

## Architecture Direction

Keep protocol primitives independent from application ingress. Browser/WASM
support should be a separate crate that reuses pure protocol pieces where
possible and supplies a WebSocket/WebTransport-style transport, rather than
trying to compile the Tokio TCP local/remote binaries to wasm.
