# Changelog

## Unreleased

## v0.0.5

`v0.0.5` hardens the native mux beta and cleans up shared runtime plumbing after
the v0.0.4 release candidate.

### Added

- Native mux beta behavior: PING RTT measurement, graceful GOAWAY drain,
  priority-aware DATA scheduling, bounded per-stream send queues, parser fuzz
  target, and JSON mux benchmark output.
- Local admin runtime reload/apply. `espejismo-local` can rebuild its tunnel
  pool and update server, local auth, TCP/pacing/obfuscation, mux mode, and
  tunnel-pool settings for new flows without restarting the process.
- Release workflow checksum generation and draft GitHub release creation on tag
  pushes.

### Fixed

- Standard handshakes now use a variable-length masked envelope for both client
  hello and server reply, removing fixed cleartext field offsets from the
  default `balanced` profile while preserving robust length-based parsing.
- macOS route manager now imports the split parser types required by darwin
  release builds.
- Native mux batches WINDOW_UPDATE frames until buffered payload is consumed and
  reports writes after peer FIN/RST as `BrokenPipe` instead of silently dropping
  data.
- Admin token checks use constant-time comparison.
- TUN UDP relay tasks and remote mux stream tasks are bounded to reduce local
  resource exhaustion risk.
- Runtime state and user metrics recover poisoned mutexes instead of panicking
  through the admin/status path.
- Native mux implementation is split into frame codec, pending queue, and test
  modules so the session state machine stays smaller and easier to audit.
- Native mux command and accept channels are bounded, and pending outbound
  frames have a session-level cap derived from stream limits.
- DATA for an unknown native stream now emits RST for that stream instead of
  terminating the whole mux session.
- Shared `FrameOptions` construction now lives in the core config model, and
  client/server bidirectional copy paths share the same metered copy loop.
- Normal frame chunk bounds now reserve space for the frame type byte and AEAD
  tag, preventing bulk/custom chunk policies from producing oversized frames.
- Windows and Ubuntu setup templates now include `chunk_policy` and carry
  obfuscation/stealth settings into generated client profiles.
- Multi-user handshakes now bind to the selected user during envelope parsing
  instead of sequentially verifying users until the first match.
- Frame receive paths avoid an extra plaintext payload allocation, and TCP
  relay copy buffers now use 32 KiB chunks for fewer syscalls on large flows.
- Tunnel lane reconnects no longer hold the primary mux-control lock while
  dialing and handshaking a replacement physical connection.

### Clarified

- Wire design does not impersonate TLS, HTTP/2, QUIC, or another named protocol;
  Espejismo keeps its own authenticated encrypted byte stream and avoids stable
  cleartext markers.

## v0.0.4

`v0.0.4` focuses on production hardening for the TCP-first tunnel path, native
TUN takeover, initial native mux hardening, tunnel pool scheduling, and release
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
- Native mux selectable with `[shared.mux].mode = "native"` while yamux
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
