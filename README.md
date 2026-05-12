# Espejismo

**[🇬🇧 English](README.md) &nbsp;|&nbsp; [🇪🇸 Español](README_ES.md)**

<p>
  <img src="https://img.shields.io/badge/Rust-1.75%2B-dea584?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Tokio-async_runtime-ff5e00?logo=tokio&logoColor=white" alt="Tokio">
  <img src="https://img.shields.io/badge/XChaCha20--Poly1305-AEAD-4a90d9" alt="AEAD">
  <img src="https://img.shields.io/badge/X25519-key__exchange-e97326" alt="X25519">
  <img src="https://img.shields.io/badge/License-MIT-blue" alt="License">
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey" alt="Platform">
</p>

A native cross-platform Rust encrypted transport tunnel for public and untrusted
networks. SOCKS5, HTTP, and optional native TUN local ingress, X25519 forward
secrecy, XChaCha20-Poly1305 encrypted frames, selectable logical stream
multiplexing, adaptive padding, TCP-friendly pacing, and client puzzles.

Current release: `v0.0.6`.

## Architecture

### System View

```mermaid
flowchart LR
    subgraph Local["espejismo-local"]
        APP["Application"]
        SOCKS["SOCKS5 ingress<br/>TCP CONNECT + UDP ASSOCIATE"]
        HTTP["HTTP proxy ingress<br/>CONNECT + absolute-form HTTP"]
        AUTH["Optional local auth"]
        YMUX_C["mux client session<br/>logical streams"]
        ENC_C["Encrypted transport adapter"]
    end

    subgraph Wire["Public TCP connection"]
        FLOW["AEAD-protected byte stream<br/>standard or stealth profile"]
    end

    subgraph Remote["espejismo-remote"]
        PROBE["Probe guard<br/>HTTP fallback or silent tarpit"]
        HS["Handshake verifier<br/>HMAC + X25519 + puzzle + replay cache"]
        ENC_R["Encrypted transport adapter"]
        YMUX_R["mux server session"]
        REQ["Tunnel request parser"]
        POLICY["Egress policy<br/>host + port ACL + SOCKS5 chain"]
        DEST["TCP / UDP destination"]
    end

    APP --> SOCKS --> AUTH --> YMUX_C
    APP --> HTTP --> AUTH
    YMUX_C --> ENC_C --> FLOW --> PROBE --> HS --> ENC_R --> YMUX_R
    YMUX_R --> REQ --> POLICY --> DEST
```

The important layering detail is that the selected mux owns logical streams,
while `spawn_frame_transport` provides it with a normal `AsyncRead + AsyncWrite`
object backed by encrypted frames. Local proxy requests become mux streams; the
physical socket carries only the encrypted transport. `yamux` is the stable
default, and the in-tree native mux can be enabled for beta testing.

### Protocol Stack

```text
Application traffic
  -> SOCKS5 / HTTP proxy parser
  -> optional local proxy auth
  -> mux logical stream
  -> encrypted frame transport
  -> TCP socket
  -> remote handshake / replay / probe defenses
  -> mux stream handler
  -> egress policy
  -> TCP connect or one-shot UDP relay
```

### Handshake

Standard mode starts with a variable-length masked handshake envelope:

```text
[ random nonce 24 ][ masked payload length 4 ][ masked payload + random tail padding ]

masked payload:
[ HMAC-SHA256 ][ UTC timestamp ][ nonce ][ X25519 public key ]
[ protocol version ][ capabilities ][ puzzle nonce ][ padding length ][ padding ]
```

The payload length and payload are XOR-masked with HMAC-derived streams keyed by
the PSK auth key, so the wire does not expose a stable HMAC/timestamp/public-key
offset or a fixed server-reply size. Inside the masked envelope, the client
solves a bounded SHA-256 leading-zero puzzle over the body before computing the
HMAC. The remote verifies the puzzle, checks timestamp skew, validates the HMAC
in constant time, and records the ephemeral public key in a bounded replay
cache. Session keys are derived with X25519 and HKDF-SHA256. The handshake
also negotiates mux capability, so a `yamux` client and `native` server fail
early with a clear protocol error instead of parsing each other incorrectly.

When `profile = "stealth"`, the hello exchange is wrapped in two fixed-size
blocks that match `shared.stealth.frame_size`. The block payload is masked with
an HMAC-derived XOR stream and random padding, so the handshake does not expose
the standard-mode envelope length.

### Frame Transport

Standard profiles use masked length-prefixed AEAD frames:

```text
[ masked ciphertext length 4 ][ XChaCha20-Poly1305(type || payload) ]
```

`low_latency`, `balanced`, and `high_entropy` tune chunk randomization, jitter,
and adaptive padding around that standard frame format.

Long-lived physical tunnels rotate frame traffic keys with an encrypted
`KEY_UPDATE` control frame after `shared.key_update_frames` transmitted frames.
The control frame is AEAD-protected under the current key, then both sides
derive the next traffic secret and length-mask key with HKDF.

Stealth mode uses fixed-size AEAD frames without a length header:

```text
[ XChaCha20-Poly1305 ciphertext exactly shared.stealth.frame_size bytes ]

plaintext before encryption:
[ type 1 ][ payload_len 2 ][ payload ][ random padding to fixed size ]
```

The upload pump sends a short random padding warmup after the stealth handshake,
then writes data or padding frames on a paced schedule. If no application data
is queued, the idle cadence decays from the base `tick_ms` toward slower
heartbeat-like intervals; real data resets the cadence. A small pre-write jitter
is applied so data and padding frames do not have perfectly identical scheduler
behavior.

### Probe And Fallback Behavior

Unknown or invalid peers receive no protocol error. Depending on remote config,
they are either held in a bounded silent tarpit or, for HTTP-looking probes,
routed to a configured fallback upstream. If no upstream is configured, the
built-in fallback returns a small HTTP 200 response with dynamic `Date`,
`Last-Modified`, `ETag`, `Content-Length`, `Connection`, and `Server` headers.
A real Nginx/Caddy upstream is still the preferred production fallback because
it inherits a complete and natural web-server fingerprint.

### What Stealth Helps With

| Observable signal | Mitigation in this codebase | Remaining caveat |
| --- | --- | --- |
| Plain handshake size | Stealth wraps hello/reply in fixed-size masked blocks | First two blocks are still connection-start metadata |
| Frame size distribution | All stealth data, close, and padding frames use one size | Fixed-size flows can themselves be unusual |
| Burst/silence behavior | Padding frames continue when no app data is queued | Idle cadence deliberately decays to reduce constant-stream fingerprints |
| Direction asymmetry | Both sides run the same stealth transport behavior | Kernel scheduling and congestion can still differ by direction |
| Payload classification | AEAD hides frame type and content | Traffic volume, endpoint, and duration remain visible |
| Active HTTP probes | Optional upstream fallback or dynamic built-in response | Built-in fallback is a convenience, not a substitute for a real website |

Stealth is a traffic-shaping profile, not a mathematical guarantee of
undetectability. It reduces several obvious protocol fingerprints, but network
observers can still model metadata such as endpoint reputation, connection
duration, total byte volume, retry behavior, and congestion effects.

## Platform Support

| Platform | Arch | Status |
| --- | --- | --- |
| Linux | amd64, 386, arm64, armv7 | Supported |
| macOS | Apple Silicon (arm64) | Supported |
| Windows | amd64, 386, arm64 | Supported |

## Download

Normal users do not need Rust or Cargo. Download a release archive for your
platform, extract it, and run the binaries in `bin/`.

Release artifacts:

- `linux-amd64`
- `linux-386`
- `linux-arm64`
- `linux-armv7`
- `darwin-arm64`
- `windows-amd64`
- `windows-386`
- `windows-arm64`

Each archive contains:

- `bin/espejismo-local`
- `bin/espejismo-remote`
- `configs/espejismo.toml`
- README, architecture, deployment, user, update, status, and testing notes

## Quick Start For Users

These commands use downloaded release binaries. Normal users do not need Rust,
Cargo, or a source checkout.

### Fastest Binary Start

Remote server, Linux/macOS:

```bash
ESPEJISMO_PSK='change-me-long-random-secret' \
./bin/espejismo-remote --listen 0.0.0.0:6690
```

Local client, Linux/macOS:

```bash
ESPEJISMO_PSK='change-me-long-random-secret' \
./bin/espejismo-local \
  --server remote.example.com:6690 \
  --socks5-listen 127.0.0.1:6680 \
  --http-listen 127.0.0.1:6681
```

Remote server, Windows PowerShell:

```powershell
$env:ESPEJISMO_PSK = "change-me-long-random-secret"
.\bin\espejismo-remote.exe --listen 0.0.0.0:6690
```

Local client, Windows PowerShell:

```powershell
$env:ESPEJISMO_PSK = "change-me-long-random-secret"
.\bin\espejismo-local.exe --server remote.example.com:6690 --socks5-listen 127.0.0.1:6680 --http-listen 127.0.0.1:6681
```

Then point applications at `127.0.0.1:6680` as a SOCKS5 proxy or
`127.0.0.1:6681` as an HTTP proxy. For production, prefer a TOML config or an
`espejismo://import/...` profile so secrets and user settings are not kept in
shell history.

### Linux Server

Download and extract the Linux release archive on the server, then run:

```bash
./bin/espejismo-remote --config configs/espejismo.toml
```

Or install the remote endpoint on Ubuntu with one command:

```bash
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install-ubuntu-remote.sh \
  | sudo bash
```

The installer downloads the latest GitHub release, generates a random PSK,
installs `espejismo-remote` as a systemd service, and prints a ready-to-import
`espejismo://import/...` client profile. If the auto-detected public endpoint is
not the address your client should dial, provide it explicitly:

```bash
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install-ubuntu-remote.sh \
  | sudo ESPEJISMO_PUBLIC_ENDPOINT=203.0.113.10:6690 bash
```

Generate a tuned starter config instead of hand-editing every transport knob:

```bash
./bin/espejismo-local --profile balanced --print-example-config > espejismo.toml
./bin/espejismo-remote --profile server-safe --print-example-config > espejismo-server.toml
```

### macOS Client

Download and extract the macOS release archive, edit `configs/espejismo.toml`
so `local.server` points to your remote server, then run:

```bash
./bin/espejismo-local --config configs/espejismo.toml
```

Use these local proxy endpoints:

```text
SOCKS5:     127.0.0.1:6680
HTTP proxy: 127.0.0.1:6681
```

### Windows Client

Download and extract the Windows release archive, then run PowerShell from the
extracted directory:

```powershell
.\bin\espejismo-local.exe --config .\configs\espejismo.toml
```

Or generate a local config from an import profile:

```powershell
.\scripts\setup-windows.ps1 -Mode local -ProfileUrl "espejismo://import/..."
```

### One-Line Config Import

Both binaries can run from a one-line base64 config, useful for panels and
copy/paste deployment:

```bash
CONFIG_B64="$(./bin/espejismo-local --config configs/espejismo.toml --print-config-base64)"
./bin/espejismo-local --config-base64 "$CONFIG_B64"
./bin/espejismo-local --decode-config-base64 "$CONFIG_B64" > espejismo.toml
```

### Update Check

```bash
./bin/espejismo-local --check-update
./bin/espejismo-remote --check-update
```

See [docs/deployment/QUICKSTART.md](docs/deployment/QUICKSTART.md) for detailed
Linux, macOS, and Windows deployment flows.

## Developer Build

Developers who clone the repository need Rust/Cargo. Normal users should use
release binaries from the download section instead.

Build all binaries:

```bash
git clone https://github.com/tianrking/Espejismo.git
cd Espejismo
cargo build --release
```

The compiled binaries are written to:

```text
target/release/espejismo-local
target/release/espejismo-remote
```

Run from source during development:

```bash
cargo run --bin espejismo-remote -- --config configs/examples/espejismo.toml
cargo run --bin espejismo-local -- --config configs/examples/espejismo.toml
```

Run from source without a config file:

```bash
ESPEJISMO_PSK='change-me-long-random-secret' \
cargo run --bin espejismo-remote -- --listen 0.0.0.0:6690

ESPEJISMO_PSK='change-me-long-random-secret' \
cargo run --bin espejismo-local -- \
  --server 127.0.0.1:6690 \
  --socks5-listen 127.0.0.1:6680 \
  --http-listen 127.0.0.1:6681
```

Run the release checks used before tagging:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
./scripts/e2e_smoke.sh
```

Create a local release package:

```bash
./scripts/package-release.sh
```

Windows PowerShell:

```powershell
.\scripts\package-release.ps1
```

## Configuration

Generate a starter TOML config:

```bash
./bin/espejismo-local --print-example-config > espejismo.toml
```

The same config can be used by both binaries; each one reads the relevant
section.

```toml
[shared]
psk = "change-me-long-random-secret"
clock_skew_secs = 30
puzzle_bits = 12
max_padding = 64
jitter_ms = 0
padding_chance_percent = 35
backpressure_threshold_ms = 40
backpressure_cooldown_ms = 1000
tunnel_buffer = 1048576
idle_timeout_secs = 300
max_streams = 256
max_physical_connections = 1024

[shared.obfuscation]
profile = "balanced"
chunk_policy = "balanced"
randomize_chunks = true
min_chunk = 4096
max_chunk = 16384

[shared.mux]
mode = "yamux"
native_initial_window_bytes = 1048576
native_stream_buffer_frames = 128
native_send_queue_frames = 64
native_idle_timeout_secs = 300
native_drain_timeout_secs = 30

[shared.stealth]
frame_size = 4096
tick_ms = 50

[local]
server = "127.0.0.1:6690"
socks5_listen = "127.0.0.1:6680"
http_listen = "127.0.0.1:6681"
handshake_padding = 256

[local.auth]
username = "local-user"
password = "local-pass"

[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 1
bulk_lanes = 2
max_reconnect_attempts = 3
max_connection_age_secs = 3600

[logging]
level = "info"
format = "compact"
ansi = true
# file = "/var/log/espejismo/espejismo.log"

[admin]
# listen = "127.0.0.1:9090"
# token = "change-me-admin-token"

[remote]
listen = "0.0.0.0:6690"
handshake_timeout_ms = 3000
reject_delay_ms = 0
max_handshake_padding = 1024
replay_window_secs = 60
cold_start_delay_ms = 35
tarpit_max = 1024
tarpit_hold_secs = 300

[remote.fallback_http]
mode = "silent"
# mode = "http_fallback"
# enabled = true # legacy switch, kept for backward compatibility
upstream = "127.0.0.1:8080"
probe_timeout_ms = 250
server = "nginx"
body = "<html><head><title>It works</title></head><body><h1>It works</h1></body></html>"

[[remote.users]]
name = "default"
psk = "change-me-long-random-secret"

[remote.users.quota]
# bytes = 536870912
window_secs = 86400

[remote.users.bandwidth]
# bytes_per_sec = 1048576

[remote.egress]
deny_private_ips = false
allow_hosts = []
block_hosts = []
allow_ports = []
block_ports = []
```

Run from a file:

```bash
./bin/espejismo-remote --config espejismo.toml
./bin/espejismo-local --config espejismo.toml
```

Run from a packaged release:

```bash
./bin/espejismo-remote --config configs/espejismo.toml
./bin/espejismo-local --config configs/espejismo.toml
```

Windows packaged release:

```powershell
.\bin\espejismo-remote.exe --config .\configs\espejismo.toml
.\bin\espejismo-local.exe --config .\configs\espejismo.toml
```

Windows setup helper:

```powershell
.\scripts\setup-windows.ps1 -Mode local -Server "YOUR_SERVER_IP:6690" -Psk "the-same-psk"
```

Run from base64-encoded TOML, useful for deployment panels or one-line imports:

```bash
CONFIG_B64="$(base64 -w0 espejismo.toml)"
./bin/espejismo-remote --config-base64 "$CONFIG_B64"
./bin/espejismo-local --config-base64 "$CONFIG_B64"
```

Espejismo can also convert configs without shell-specific base64 flags:

```bash
CONFIG_B64="$(./bin/espejismo-local --config espejismo.toml --print-config-base64)"
./bin/espejismo-local --decode-config-base64 "$CONFIG_B64" > espejismo.toml
```

You can print an example directly as base64:

```bash
./bin/espejismo-local --print-example-config-base64
```

Check for a newer release:

```bash
./bin/espejismo-local --check-update
./bin/espejismo-remote --check-update
```

## Handshake

The handshake protocol is described in detail in the [Architecture](#architecture)
section above, covering both plain and stealth modes. Additional protocol
internals and wire format specification live in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Notes

- `espejismo-local --socks5-listen` enables the local SOCKS5 proxy.
- `espejismo-local --http-listen` enables the local HTTP proxy.
- `[local.tun]` enables optional native TUN ingress for system-level traffic
  capture. It turns TCP flows and UDP datagrams from the virtual interface into
  the existing encrypted TCP mux tunnel. Route and DNS takeover are explicit
  Linux, macOS, and Windows opt-in settings under `[local.tun.route]`; see
  [docs/deployment/TUN.md](docs/deployment/TUN.md).
- `espejismo-local --tun-route-cleanup` restores saved TUN route/DNS state after
  a crash or service-manager stop hook. It is available on Linux, macOS, and
  Windows and is intended for `systemd ExecStopPost` or manual recovery.
- `[local.auth]` enables local SOCKS5 username/password auth and HTTP Basic
  proxy auth. Omit it for a trusted loopback-only no-auth listener.
- `[logging]` controls structured logs. `format` can be `compact`, `pretty`, or
  `json`; `file` writes logs to a path instead of stderr.
- `--log-level`, `--log-format`, `--log-file`, and `--no-log-ansi` override the
  logging config for either binary.
- `[admin]` enables an HTTP admin endpoint with `/healthz`, `/status`,
  `/connections`, `/metrics`, and remote-side runtime `/reload`/`/apply`. Use
  `token` outside trusted loopback-only environments.
- `/status`, `/connections`, and `/metrics` include lane RTT samples, session
  age, session/key rotation counters, stream open failure reasons, and egress
  deny counters for troubleshooting long-running deployments.
- `[remote.egress]` controls server-side outbound policy with host and port
  allow/block lists.
- `local.server` and `--server` accept either `ip:port` or `domain:port`; the
  local client resolves the name before opening the physical tunnel.
- `[shared.tcp]` controls TCP_NODELAY, keepalive, heartbeat padding frames,
  send/receive buffers, and optional Linux TCP_USER_TIMEOUT / congestion
  control such as `bbr` or `cubic`.
- `[shared.pacing]` enables TCP-friendly write pacing. `max_bytes_per_sec = 0`
  keeps throughput unlimited while still allowing burst and coalescing knobs.
- `[local.tunnel_pool]` keeps multiple physical TCP tunnels available. New
  streams are assigned to interactive or bulk lanes by health score so small
  proxy requests do not queue behind large TUN or download flows.
  `max_reconnect_attempts` bounds per-request reconnect attempts before the
  local proxy returns a clear error. `max_connection_age_secs` rotates physical
  tunnel sessions for new streams so long-running clients periodically perform a
  fresh X25519/HKDF handshake without interrupting existing streams.
- `[shared.mux]` selects the logical stream multiplexer. `yamux` is the stable
  default; `native` enables the in-tree beta mux for testing and benchmarking.
  The native mux uses byte-window flow control, bounded per-stream receive
  queues, bounded command/pending queues, a max-stream limit, RST for unknown
  stream DATA, and an idle GOAWAY timeout.
- `shared.key_update_frames` controls frame-level traffic-key rotation inside a
  long-lived physical tunnel.
- `shared.max_physical_connections` caps concurrently accepted remote-side
  physical TCP connections. `shared.max_streams` caps logical mux streams and
  the remote global stream semaphore.
- `[[remote.users]]` enables multiple independent server users, each with its
  own PSK. If no users are configured, the server falls back to `shared.psk`.
- `[remote.users.quota]` sets an optional per-user rolling byte quota. `bytes`
  is disabled when omitted; `window_secs` defaults to 86400.
- `[remote.users.bandwidth]` sets an optional per-user aggregate byte-per-second
  limit across TCP and UDP relay traffic. See
  [docs/deployment/USERS.md](docs/deployment/USERS.md).
- `[remote.egress].socks5_proxy` optionally chains TCP egress and UDP egress
  through another SOCKS5 proxy. UDP uses SOCKS5 UDP ASSOCIATE.
- `espejismo-local --print-client-profile` emits an
  `espejismo://import/...` profile URL that can be imported with
  `--import-profile`.
- `--print-config-base64` prints the selected TOML config as a one-line base64
  string. `--decode-config-base64` prints that string back as TOML.
- `--check-update` checks the latest release metadata and prints whether a newer
  version is available. `--update-url` can point at a compatible JSON endpoint
  with `tag_name` or `latest_version`. See
  [docs/deployment/UPDATES.md](docs/deployment/UPDATES.md).
- `--check-config --config espejismo.toml` validates common deployment mistakes:
  DNS resolution, listener bindability, weak PSKs, admin token exposure, egress
  breadth, user duplication, quotas, bandwidth, stream limits, clock skew,
  handshake timeouts, and pacing bounds.
- SOCKS5 supports TCP `CONNECT` and UDP `ASSOCIATE`. UDP datagrams are relayed
  over authenticated mux streams and checked by remote egress policy.
- The stable production path is TCP with `shared.mux.mode = "yamux"`. SOCKS5 UDP ASSOCIATE is currently an
  application-level UDP relay over that TCP tunnel; physical UDP underlay code is
  reserved for experiments and is not the recommended deployment mode.
- `--max-padding` controls the maximum payload size of encrypted padding frames.
- `--padding-chance-percent` controls how often padding is attempted.
- `--backpressure-threshold-ms` detects slow writes and disables padding.
- `--backpressure-cooldown-ms` controls how long padding stays disabled after a
  slow write.
- `--jitter-ms` applies a small randomized delay before outgoing frames.
- `[shared.obfuscation]` controls sender-side traffic shape. `profile` can be
  `low_latency`, `balanced`, `high_entropy`, `bulk`, or `stealth`.
  `chunk_policy` selects adaptive encrypted data chunks: `low_latency` uses
  2-8 KiB, `balanced` uses 4-16 KiB, `bulk` uses large chunks capped just below
  64 KiB to leave room for frame metadata and AEAD tag, `stealth` uses the fixed
  stealth payload capacity, and `custom` uses `min_chunk` / `max_chunk`.
- `[shared.stealth]` is used when `profile = "stealth"`: every encrypted frame
  is exactly `frame_size` bytes. The transport starts with a short random
  padding warmup, sends data or padding on a paced cadence, and gradually slows
  idle padding toward heartbeat-like intervals before real data resets it.
- `--puzzle-bits` configures the client puzzle difficulty. Values are capped at
  24 bits.
- `espejismo-local --handshake-padding` controls the maximum random padding in
  the first packet.
- `espejismo-remote --max-handshake-padding` limits accepted first-packet
  padding.
- `espejismo-remote --replay-window-secs` controls the in-memory replay cache
  window.
- `espejismo-remote --handshake-timeout-ms` bounds incomplete handshakes.
- `espejismo-remote --reject-delay-ms` adds a bounded silent close delay for
  invalid handshakes. Values above 10000 ms are capped.
- `espejismo-remote --tarpit-max` controls the bounded silent tarpit size used
  when `reject_delay_ms = 0`.
- `espejismo-remote --tarpit-hold-secs` controls how long invalid sockets are
  retained in the tarpit before close.
- `[remote.fallback_http]` controls active-probe behavior. Use
  `mode = "silent"` for bounded silent handling, or `mode = "http_fallback"`
  to route HTTP probe prefixes to either a configured
  `upstream` TCP endpoint (for example local nginx) or an internal 200 OK page.
- `--tunnel-buffer` controls the in-process encrypted transport buffer used
  below the logical stream mux.
- `espejismo-remote --cold-start-delay-ms` applies a small startup delay after
  a valid handshake and before the mux begins.
- The PSK accepts `hex:...`, `base64:...`, or a raw UTF-8 string.
- Invalid handshakes are closed quietly by default. With
  `[remote.fallback_http].enabled = true`, probes receive HTTP-looking fallback
  responses instead.
- The tarpit is intentionally silent: it holds sockets briefly and never sends
  drip bytes to unknown peers.

## Smoke Test

```bash
./scripts/e2e_smoke.sh
REQUESTS=200 CONCURRENCY=32 ./scripts/stress_smoke.sh
MUX_MODE=native ./scripts/e2e_smoke.sh
```

On Windows PowerShell:

```powershell
.\scripts\e2e_smoke.ps1
.\scripts\stress_smoke.ps1 -Requests 200 -Concurrency 16
```

The script starts a local HTTP server, `espejismo-remote`, and `espejismo-local`,
then performs SOCKS5 TCP, SOCKS5 UDP, HTTP proxy, HTTP CONNECT, admin, metrics,
and profile import checks through the encrypted mux tunnel.
The stress script adds single-stream, high-concurrency small-request,
mixed-lane, remote-restart, and optional soak coverage.

## Logging

Console logs default to compact human-readable output:

```toml
[logging]
level = "info"
format = "compact"
ansi = true
```

For production ingestion, use JSON logs:

```toml
[logging]
level = "info,espejismo_core=debug"
format = "json"
ansi = false
file = "/var/log/espejismo/espejismo.log"
```

The `level` field accepts normal tracing filter directives, so operators can
raise one module while keeping the rest quiet.

## Project Status

See [docs/development/STATUS.md](docs/development/STATUS.md) for the implemented
feature matrix and the remaining roadmap, including transparent migration,
WASM/browser packaging, UDP underlay socket integration, and richer
multi-profile control.

See [CHANGELOG.md](CHANGELOG.md) for release notes.

See [docs/deployment/CLI.md](docs/deployment/CLI.md) for command-line usage,
[docs/testing/TEST_PLAN.md](docs/testing/TEST_PLAN.md) for the executable test
strategy, and [docs/research/DESIGN_PRINCIPLES.md](docs/research/DESIGN_PRINCIPLES.md)
for the protocol design principles.

## Responsible Use

Espejismo is intended for encrypted access to systems you own or are explicitly
authorized to administer, such as a home lab, private server, or internal test
environment. It is not a service, anonymity network, or authorization bypass
tool.

Traffic shaping can reduce some protocol fingerprints, but it does not make a
connection invisible. Operators should assume that endpoints, timing, byte
volume, uptime, routing path, and deployment mistakes may still be observable.
Use real fallback upstreams, conservative logging, strong PSKs, and restrictive
egress policy in production.

You are responsible for complying with all applicable laws, network policies,
terms of service, export controls, and authorization boundaries in your
jurisdiction and in any network where you deploy or use this software. Do not use
Espejismo to access systems without permission, evade lawful access controls, or
violate local regulations. This README is technical documentation, not legal
advice; consult qualified counsel if your deployment has legal or compliance
risk.
