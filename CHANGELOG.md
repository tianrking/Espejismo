# Changelog

## v0.0.4

`v0.0.4` focuses on production hardening for the TCP-first tunnel path, native
TUN takeover, native mux alpha hardening, tunnel pool scheduling, and release
quality gates.

### Added

- Shared TCP socket tuning: TCP_NODELAY, keepalive, configurable socket buffers,
  Linux TCP_USER_TIMEOUT, and optional Linux congestion-control selection.
- TCP-friendly write pacing with rate caps, burst budget, minimum write size,
  heartbeat frames, and backpressure-aware padding reduction.
- Enriched admin runtime state through `/status` and `/connections`, including
  tunnel state, reconnect count, recent errors, egress policy version, config
  apply time, active streams, physical connections, and per-user bytes.
- Config diagnostics through `--check-config` for local and remote binaries.
- Optional native TUN ingress for system-level traffic capture.
- Linux, Windows, and macOS route/DNS managers for explicit TUN auto-route and
  auto-DNS takeover, with protected remote-server routes and best-effort restore
  on shutdown.
- Fuzz targets for protocol/config parsing and CI clippy enforcement.
- Replaceable mux wrapper around the current yamux implementation.
- Local TCP tunnel pool with configurable interactive and bulk physical lanes.
- Stream priority in tunnel requests so SOCKS5/HTTP interactive traffic and TUN
  bulk traffic can be scheduled separately.
- Adaptive chunk policies for low-latency, balanced, bulk, stealth, and custom
  frame sizing.
- Per-lane health data in admin status, including reconnect count, last error,
  open-stream latency, active streams, and bytes.
- Unix stress smoke script covering single-stream traffic, many small requests,
  mixed-lane traffic, remote restart recovery, and optional soak loops.
- Native mux alpha selectable with `[shared.mux].mode = "native"` while yamux
  remains the default fallback.
- Mux benchmark helper comparing yamux and native stress runs.
- Native mux resource controls: byte-window flow control, bounded per-stream
  receive queues, max-stream enforcement, idle GOAWAY shutdown, and session
  task abort on drop.
- Configurable per-request tunnel reconnect attempt limit.

### Changed

- Refactored large local, remote, and configuration modules into clearer runtime
  modules with separate handlers, relay helpers, route managers, tunnel manager,
  and config type/default files.
- Updated deployment, packaging, TUN, CLI, status, testing, English README, and
  Spanish README documentation to match the current TCP-first, cross-platform
  release shape.
- Smoke tests now verify release update checks against the `0.0.4` binary
  version and exercise platform-aware TUN config diagnostics.

### Known Limits

- The stable production underlay remains TCP/yamux. SOCKS5 UDP ASSOCIATE and
  TUN UDP datagrams are relayed at the application layer over the encrypted TCP
  tunnel; the physical UDP underlay remains experimental core code.
- Native route/DNS takeover requires elevated OS privileges and may need manual
  adjustment on hosts with unusual route tables, VPN clients, or managed DNS
  policy.

## v0.0.2

`v0.0.2` turns Espejismo from a minimal encrypted proxy prototype into a more
operable native tunnel release.

### Added

- Multi-user remote authentication with independent per-user PSKs.
- Per-user rolling byte quotas and aggregate bandwidth limits.
- Server-side SOCKS5 chained egress for TCP and UDP. TCP uses CONNECT; UDP uses
  SOCKS5 UDP ASSOCIATE.
- Remote admin runtime management:
  - `POST /reload` re-reads the startup config source.
  - `POST /apply` accepts a TOML config body and applies runtime settings.
- Config import/export CLI:
  - `--print-config-base64`
  - `--decode-config-base64`
  - existing `--config-base64`
- Client profile export/import through `espejismo://import/...`.
- Release update checks for both binaries with `--check-update` and
  configurable `--update-url`.
- UDP underlay core primitives: packet codec, session id, sequence numbers,
  cumulative ACK, retransmission scheduling, and congestion window logic.
- Deployment docs for users, egress, admin, update checks, packaging, logging,
  profiles, and quick start.

### Changed

- Release packages now include the expanded deployment documentation set.
- Smoke tests now verify config conversion, update checks, runtime `/apply`,
  SOCKS5 TCP, SOCKS5 UDP, HTTP proxy, HTTP CONNECT, admin, metrics, profiles,
  and packaging.

### Known Limits

- The stable physical tunnel still uses TCP/yamux. UDP underlay socket
  integration is prepared in core primitives but is not yet the default runtime
  transport.
- Runtime apply affects new physical tunnels and newly opened logical streams.
  Process-owned resources such as listener addresses and log file handles still
  require restart.

## v0.0.1

- Initial native Rust workspace.
- Encrypted TCP physical tunnel with yamux logical streams.
- SOCKS5 and HTTP local proxy ingress.
- X25519 key exchange, XChaCha20-Poly1305 frames, replay cache, client puzzle,
  adaptive padding, stealth profile, admin metrics, packaging, and smoke tests.
