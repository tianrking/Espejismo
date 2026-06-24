# Changelog

## Unreleased

## v0.1.2

`v0.1.2` is a throughput observability and lane-dispatch release for the
`v0.1.x` line.

### Added

- `scripts/bench-throughput.sh` for repeatable direct/proxy download and upload
  benchmarks with raw curl output, JSONL results, Markdown summaries, and
  optional admin snapshots.
- Per-lane runtime counters in admin status and Prometheus metrics, including
  active streams, pending stream opens, streams opened, stream open failures,
  per-lane bytes, last activity time, and session age.
- HK2 to RK benchmark records documenting direct-link variance, Espejismo
  overhead, and lane-dispatch behavior under parallel upload tests.

### Changed

- Concurrent local stream opens now reserve a lane before connecting so
  simultaneous bulk flows spread across available tunnel lanes.
- Lane scoring now prefers idle lanes over lower-latency loaded lanes, with open
  latency used only as a tie-breaker after load.

### Fixed

- Fixed Clippy `items_after_test_module` CI failure by keeping the handler test
  module at the end of the file.
- Fixed parallel bulk upload imbalance where several streams could land on the
  same lane before `active_streams` was updated.

## v0.1.1

`v0.1.1` is a TUN stability and operations release for the `v0.1.x` line.

### Added

- Local TUN UDP controls:
  `local.tun.udp_enabled`, `local.tun.udp_timeout_secs`, and
  `local.tun.udp_block_ports`.
- CLI overrides for TUN UDP behavior: `--tun-disable-udp`,
  `--tun-udp-timeout-secs`, and `--tun-udp-block-ports`.

### Changed

- Handshake authentication now supports dynamic time-window HKDF keys through
  `[shared.handshake_window]`. Clients send with the current window; servers
  accept the configured adjacent windows and otherwise fall back to the existing
  silent reject/tarpit path for invalid first packets.
- Remote egress can now chain through `remote.egress.proxy` using SOCKS4,
  SOCKS4a, SOCKS5, HTTP CONNECT, or HTTPS CONNECT upstreams. SOCKS5 supports
  optional username/password authentication and UDP ASSOCIATE. HTTP/HTTPS
  CONNECT supports optional Basic auth. The legacy `socks5_proxy` field remains
  supported.
- TUN TCP and UDP relay streams now use interactive lanes directly instead of
  trying bulk lanes first, avoiding bulk-lane open timeout bursts during global
  desktop route takeover.
- TUN UDP relay defaults to a shorter response timeout and blocks UDP/443 by
  default so browsers fall back from QUIC to TCP HTTPS.

### Fixed

- Reduced TUN UDP/QUIC timeout storms seen with global macOS TUN mode.
- Reduced local TUN startup pressure that could cascade into broken pipes and
  unstable relay behavior under many simultaneous system flows.

## v0.1.0

`v0.1.0` is the first operational release line for the simplified packaging,
aligned tunnel request wire format, release installer flow, and documented
cross-platform TUN deployment path.

### Changed

- Installer scripts have been reduced to thin release-package downloaders:
  `scripts/install.sh` for Linux/macOS/Git Bash and `scripts/install.ps1` for
  Windows PowerShell.
- Release packages now include only the binaries, example config, docs, and the
  two download installers.
- Tunnel request wire format now matches the protocol specification:
  command, priority, authority, and optional UDP payload.

### Fixed

- Public admin listeners now require `admin.token` at config validation time.

## v0.0.9

`v0.0.9` focuses on featureless-chaos hardening and release-doc alignment.

### Added

- Per-session stealth frame-size diversification through
  `shared.stealth.frame_size_candidates`.
- Config validation for stealth frame-size candidates (bounds, minimum viable
  encrypted payload, duplicate rejection).
- Coverage for stealth frame-size candidate validation and deterministic
  session selection.

### Changed

- Stealth transport now selects fixed frame size per authenticated session
  instead of using a single global deployment-wide size.
- Default and local starter config templates now include
  `shared.stealth.frame_size_candidates`.
- Release documentation, checklists, and packaging/tag instructions are aligned
  to `v0.0.9`.

## v0.0.8

`v0.0.8` focuses on release-readiness for the low-feature transport direction
and TUN reliability checks.

### Added

- Protocol specification documenting the no-impersonation, low-feature
  encrypted transport model.
- Extension traits for authentication, outbound connectors, request policy,
  traffic observation, and transport connectors.
- `--doctor` diagnostics for low-feature configuration risks and TUN deployment
  checks.
- `--probe-server` for local TCP plus Espejismo handshake probing.
- Structured traffic observation events on the remote relay path.

### Changed

- Local physical tunnel dialing now goes through `TransportConnector`, keeping
  TCP as the default connector while making future underlays explicit.
- Release packaging now includes the protocol specification.
- TUN documentation now states the current IPv4 global-forwarding scope,
  route-recovery behavior, and ordinary proxy versus TUN mode boundary.

## v0.0.7

`v0.0.7` focuses on making Linux TUN takeover usable in real desktop routing
setups while keeping the `v0.0.6` protocol and installer model intact.

### Changed

- Linux TUN takeover now uses a dedicated policy-routing table instead of
  rewriting the `main` default route.
- Remote server endpoint traffic is protected with a high-priority direct rule
  through the existing `main` routing table before ordinary traffic is sent to
  TUN.
- `espejismo-local` warms up an interactive tunnel stream before route/DNS
  takeover, so the physical tunnel is already alive before the first DNS burst
  enters TUN.
- TUN stream opening now falls back from bulk lanes to interactive lanes and
  reports open timeouts clearly.

### Fixed

- Disabled misleading local ICMP echo handling in TUN mode. Use TCP probes such
  as `curl http://1.1.1.1/cdn-cgi/trace` instead of `ping` for TUN validation.
- Added clearer TUN TCP/UDP flow diagnostics so route, tunnel-open, and relay
  failures can be distinguished from DNS issues.

## v0.0.6

`v0.0.6` completes the protocol-operations upgrade and makes the installer path
usable for normal server/client deployment without hand-editing config.

### Added

- Protocol operations upgrade: mux mode negotiation, frame-level key updates,
  key material zeroization, config profiles, route cleanup helpers, richer
  tunnel/session metrics, and JSON benchmark output.
- Reversible client onboarding conversion:
  - TOML config to `espejismo://import/...` profile with
    `--print-client-profile`.
  - Profile URL back to TOML with `--print-config` or `--write-config`.
- Guided Linux/macOS and Windows installers that download the latest GitHub
  Release artifact for the current platform.
- Remote installer public endpoint detection. Remote installs can use
  `ESPEJISMO_PUBLIC_ENDPOINT=host:port`, `ESPEJISMO_PUBLIC_HOST=host`, or
  automatic public IP detection.
- Installer manager `connect` command that prints browser/app proxy settings,
  curl test commands, and remote client import profiles.

### Changed

- Root Linux non-interactive `install.sh | sudo bash` now defaults to the
  `remote` server role; non-root installs default to `local`.
- Local SOCKS5/HTTP proxy authentication is disabled by default because the
  generated listeners bind to `127.0.0.1`. Set
  `ESPEJISMO_LOCAL_AUTH_PASSWORD` or `ESPEJISMO_CLIENT_AUTH_PASSWORD` to enable
  optional local proxy auth.
- Installer-written configs are role-specific. Remote installs keep client-only
  `[local]` settings out of the server config and generate them only in the
  printed client import profile.
- Re-running the installer restarts the selected role so printed credentials
  and config match the running process immediately.

### Fixed

- `curl | bash` non-interactive installs no longer exit silently on prompt
  reads.
- Installer random secret generation now has explicit OpenSSL, Python, and
  `/dev/urandom` fallbacks with clear errors.
- Remote installers reject invalid public endpoints such as
  `0.0.0.0:6690` and endpoint strings without a port.
- Browser onboarding no longer fails by default due to SOCKS5/HTTP proxy auth
  prompts that many browsers do not handle well.

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
- Remote-side resource limits now include a configurable physical connection
  cap, bounded stream-permit waits, bounded initial tunnel-request reads, and
  bounded admin/HTTP proxy header reads.
- Native mux now fail-fasts stream-id exhaustion instead of saturating into a
  possible stream-id collision.
- Config validation now rejects zero or excessive stream/connection limits,
  non-positive clock skew/replay/timeout values, and too-small stealth frames.
- Remote reload no longer keeps CLI PSK/admin-token overrides in the long-lived
  runtime state, and admin reload/apply errors return generic client-facing
  messages.
- Local tunnel lanes now rotate physical connections for new streams after
  `local.tunnel_pool.max_connection_age_secs`, giving long-lived clients fresh
  X25519/HKDF sessions without interrupting existing streams.

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
