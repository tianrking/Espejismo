# Implementation Status

Current release target: `v0.0.2`.

## Implemented

- Native Rust workspace with separate local, remote, and shared core crates.
- SOCKS5 local proxy ingress with optional username/password authentication.
- SOCKS5 UDP ASSOCIATE relay for request/response datagrams.
- UDP underlay packet codec and reliability core with sequence numbers, ACKs,
  cumulative delivery, and retransmission scheduling.
- UDP underlay congestion-control core with slow-start, additive growth, and
  loss backoff.
- HTTP proxy ingress with optional Basic proxy authentication, CONNECT, and
  absolute-form HTTP forwarding.
- Shared TOML configuration and base64 configuration import.
- CLI config conversion with `--print-config-base64` and
  `--decode-config-base64`.
- Client profile export/import through `espejismo://import/...` URLs.
- Multi-user remote authentication with independent per-user PSKs and user
  metrics.
- Per-user rolling byte quotas and aggregate relay bandwidth limits.
- Configurable compact, pretty, or JSON logging with optional file output.
- Admin HTTP endpoint with health, status, Prometheus-style metrics, and reload.
- Remote runtime config reload/apply through the authenticated admin endpoint.
- Client and remote release update checks with configurable metadata URL.
- Server-side egress policy for host/port allow and block rules.
- SOCKS5 chained TCP egress and SOCKS5 UDP ASSOCIATE chained UDP egress.
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
- Richer multi-profile control plane and log rotation policy.
- OS-specific TCP_INFO congestion telemetry. Current backpressure is portable write-latency based.
- UDP underlay socket integration.

## Architecture Direction

Keep protocol primitives independent from application ingress. Browser/WASM
support should be a separate crate that reuses pure protocol pieces where
possible and supplies a WebSocket/WebTransport-style transport, rather than
trying to compile the Tokio TCP local/remote binaries to wasm.
