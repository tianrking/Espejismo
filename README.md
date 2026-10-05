# Espejismo

[Español](README_ES.md) | [Quickstart](docs/deployment/QUICKSTART.md) | [FAQ](docs/deployment/FAQ.md) | [Roadmap](docs/ROADMAP.md) | [Support](docs/SUPPORT.md) | [Acknowledgments](docs/ACKNOWLEDGMENTS.md) | [Operations index](docs/deployment/INDEX.md) | [Development docs](docs/development/INDEX.md) | [Contributor onboarding](docs/ONBOARDING.md) | [Contributing](CONTRIBUTING.md) | [Brand usage](docs/BRANDING.md) | [License](LICENSE) | [Security](docs/SECURITY.md) | [Configuration](docs/deployment/CONFIG.md) | [Protocol](docs/PROTOCOL.md) | [Glossary](GLOSSARY.md)

**Start here:** [Quickstart](#quickstart-linuxmacos) · [Deployment topology](#typical-deployment-topology) · [Install](#install-from-release) · [Configuration](#one-config-file) · [Operations guides](#operations-docs) · [Build](#build-from-source)

New here? See the [FAQ](docs/deployment/FAQ.md) for common setup, networking,
and security questions.

![Release](https://img.shields.io/badge/release-v0.1.5-0b7285)
![Rust](https://img.shields.io/badge/rust-native-9a3412)
![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20macOS%20%7C%20windows-1f6feb)
![Ingress](https://img.shields.io/badge/ingress-socks5%20%7C%20http%20%7C%20tun-2f9e44)
![License](https://img.shields.io/badge/license-MIT-495057)

Espejismo is a native Rust encrypted tunnel for running private client traffic
through an authenticated remote egress server. It keeps the operational model
small: one server binary, one local client binary, one TOML configuration file,
and release archives that can be installed with a single command.

## Why Espejismo

Espejismo is a native Rust encrypted tunnel that refuses to impersonate TLS, QUIC, or any other protocol — instead it relies on authenticated encrypted chaos: masked metadata, dynamic handshake windows, padding, and silent rejection of unauthenticated probes. It keeps the operational model deliberately small: one server binary, one client binary, one TOML configuration file. If you want a tunnel you can read, audit, and understand (start with `docs/PROTOCOL.md`), it may be for you; if you want protocol camouflage or a multi-protocol suite, it is not.

## Quickstart (Linux/macOS)

This gets a SOCKS5 proxy running with the published release. You need a server
with a reachable TCP port `6690`, plus a client machine. Allow TCP `6690` in the
server firewall/security group. The commands below assume the default install
directory (`~/.espejismo`).

1. Install the full package on both machines:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh | sh
   ```

2. On the server, copy the example config:

   ```bash
   cp ~/.espejismo/configs/espejismo.toml ./espejismo.toml
   ```

   Set `[shared].psk` and the `psk` in `[[remote.users]]` to the same long
   random value, and leave `[remote].listen` at `"0.0.0.0:6690"`. Start the
   server:

   ```bash
   ~/.espejismo/bin/espejismo-remote --config ./espejismo.toml
   ```

3. On the client, copy the same example config, set `[shared].psk` to that
   identical value, and set `[local].server` to your server's public IP or
   hostname plus `:6690`:

   ```bash
   cp ~/.espejismo/configs/espejismo.toml ./espejismo.toml
   # Edit ./espejismo.toml: [shared].psk and [local].server
   ~/.espejismo/bin/espejismo-local --config ./espejismo.toml --check-config
   ~/.espejismo/bin/espejismo-local --config ./espejismo.toml --probe-server
   ~/.espejismo/bin/espejismo-local --config ./espejismo.toml
   ```

   Configure an application to use SOCKS5 at `127.0.0.1:6680` (or HTTP at
   `127.0.0.1:6681`). For example, verify with
   `curl --proxy socks5h://127.0.0.1:6680 https://example.com/`.

Keep each process running in its terminal. For Windows commands, TUN mode,
server-only packages, and deployment details, see the
[deployment quickstart](docs/deployment/QUICKSTART.md).

## Typical Deployment Topology

```text
Client machine                                      Remote server
┌─────────────────────────────┐                    ┌──────────────────────────┐
│ Applications                │                    │                          │
│   ├─ SOCKS5 ─┐              │                    │                          │
│   ├─ HTTP ───┴─> espejismo-local                  │                          │
│   └─ TUN (optional) ────────┘                    │                          │
│                       │                          │                          │
│             authenticated encrypted              │                          │
│             TCP tunnel (one or more lanes)        │                          │
└───────────────────────┼──────────────────────────┘                          │
                        └──── TCP / optional WebSocket or HTTP/2 underlay ─────>
                                                   │ espejismo-remote          │
                                                   │          │               │
                                                   │          └─> optional    │
                                                   │              upstream    │
                                                   │              proxy       │
                                                   └──────────┬───────────────┘
                                                              │
                                                              v
                                                   Destination services
```

Applications use the local SOCKS5 or HTTP listener, or optionally send system
traffic through the client's TUN interface. `espejismo-local` multiplexes those
flows over authenticated encrypted TCP tunnel lanes to `espejismo-remote`;
WebSocket and HTTP/2 are optional TCP underlays. The remote process connects to
the requested destination directly or through its configured upstream proxy.
SOCKS5 UDP relay is carried through the same TCP tunnel. The diagram describes
the normal client-to-server deployment; it does not imply protocol camouflage
or a separate UDP transport.

## Technical Profile

| Layer | What ships in `v0.1.5` |
| --- | --- |
| Client ingress | SOCKS5, HTTP proxy, and native TUN capture with configurable UDP controls |
| Remote egress | Authenticated TCP listener with configurable outbound policy |
| Transport | TCP/yamux, multi-lane pool, WebSocket underlay, HTTP/2 underlay, and deterministic port hopping |
| Cryptography | X25519 session setup, dynamic HKDF handshake windows, replay digest cache, and XChaCha20-Poly1305 protected frames |
| Routing | Linux, macOS, and Windows IPv4 TUN route/DNS takeover |
| Packaging | Cross-platform full and server-only GitHub Release archives |

Server-side egress can also chain through an upstream proxy:

```toml
[remote.egress]
proxy = "socks5://user:pass@127.0.0.1:1080"
# proxy = "http://user:pass@127.0.0.1:8080"
# proxy = "https://user:pass@proxy.example.com:8443"
```

SOCKS4/SOCKS4a, SOCKS5, HTTP CONNECT, and HTTPS CONNECT are supported for TCP
chaining. UDP chaining requires SOCKS5.

`espejismo-remote` runs on the VPS or server. `espejismo-local` runs on the
client machine and exposes local SOCKS5/HTTP proxy ports or a native TUN
interface for system-level IPv4 traffic capture.

In `v0.1.5`, TUN mode routes desktop TCP/UDP flows through interactive tunnel
lanes by default and blocks UDP/443 locally unless configured otherwise, so
browsers fall back from QUIC to TCP HTTPS instead of accumulating long UDP
timeouts.

`v0.1.5` also tightens lane observability and scheduling inputs: plain HTTP
download-looking `GET` paths use bulk lanes, and admin per-lane byte counters
include bytes from streams that are still active.

For live HK2 to RK mode data, including TCP, stealth, WebSocket, HTTP/2, and
port hopping, see [v0.1.3 HK2/RK mode matrix](docs/testing/V0.1.3_HK2_RK_MODE_MATRIX.md).
For the adaptive lane scheduler and five-round median benchmark pass, see
[HK2/RK throughput tuning](docs/testing/THROUGHPUT_TUNING_HK2_RK.md).
Browse the [performance documentation index](docs/testing/PERFORMANCE_INDEX.md)
for tuning guidance, benchmark methods, and recorded results.
These reports describe measurements, not guaranteed rates.

## Install From Release

Linux, macOS, or Windows Git Bash:

```bash
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh | sh
```

Windows PowerShell:

```powershell
iwr -useb https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.ps1 | iex
```

Installer inputs:

| Variable | Default | Purpose |
| --- | --- | --- |
| `ESPEJISMO_VERSION` | `latest` | Release tag such as `v0.1.5` |
| `ESPEJISMO_PACKAGE` | `full` | `full` for client+server, `server` for remote only |
| `ESPEJISMO_INSTALL_DIR` | `$HOME/.espejismo` | Extraction directory |
| `ESPEJISMO_REPO` | `tianrking/Espejismo` | GitHub repository |
| `ESPEJISMO_ARCHIVE_URL` | empty | Direct archive override |

The installer only downloads and extracts the matching GitHub Release package.
It does not create services, firewall rules, route changes, or hidden background
processes.

## One Config File

Use [configs/examples/espejismo.toml](configs/examples/espejismo.toml) as the
single configuration shape for both sides. The server reads `[shared]`,
`[remote]`, `[logging]`, and `[admin]`. The client reads `[shared]`, `[local]`,
`[logging]`, and `[admin]`.

Minimum server/client edit:

```toml
[shared]
psk = "change-me-to-a-long-random-secret"

[shared.handshake_window]
enabled = true
step_secs = 30
previous_windows = 1
future_windows = 0

[shared.obfuscation]
profile = "stealth"
chunk_policy = "stealth"
randomize_chunks = false

[shared.stealth]
frame_size = 4096
frame_size_candidates = [3328, 3584, 4096, 4608]
tick_ms = 20

[local]
server = "YOUR_SERVER_IP_OR_DOMAIN:6690"
socks5_listen = "127.0.0.1:6680"
http_listen = "127.0.0.1:6681"

[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 2
bulk_lanes = 2

[remote]
listen = "0.0.0.0:6690"
```

`shared.handshake_window` derives the first-packet handshake key from the PSK
and a short time slot, so recorded handshakes expire quickly. `stealth` frames
hide stable payload lengths with fixed-size encrypted blocks. The tunnel pool
spreads new logical streams across independent TCP lanes to reduce single-lane
head-of-line blocking.

Run the remote side on the server:

```bash
~/.espejismo/bin/espejismo-remote --config ~/.espejismo/configs/espejismo.toml
```

Run the local side on the client:

```bash
~/.espejismo/bin/espejismo-local --config ~/.espejismo/configs/espejismo.toml
```

Then point applications at:

```text
SOCKS5: 127.0.0.1:6680
HTTP:   127.0.0.1:6681
```

For system-level capture, start the client with TUN enabled:

```bash
sudo ~/.espejismo/bin/espejismo-local \
  --config ~/.espejismo/configs/espejismo.toml \
  --tun-enabled \
  --tun-auto-route \
  --tun-auto-dns
```

On Windows, run the terminal as Administrator. Official Windows release archives
include `bin/wintun.dll` beside `espejismo-local.exe`.

## Operations Docs

Browse the [operations documentation index](docs/deployment/INDEX.md) for the
complete guide catalog. The most common next steps are:

| Topic | Link |
| --- | --- |
| First deployment | [Deployment quickstart](docs/deployment/QUICKSTART.md) |
| Questions and troubleshooting | [FAQ](docs/deployment/FAQ.md) · [Troubleshooting index](TROUBLESHOOTING.md) · [Detailed deployment checks](docs/deployment/TROUBLESHOOTING.md) |
| Configuration and CLI | [Configuration reference](docs/deployment/CONFIG.md) · [Examples index](docs/deployment/CONFIG-EXAMPLES.md) · [CLI flags](docs/deployment/CLI.md) |
| Network capture | [Native TUN mode](docs/deployment/TUN.md) · [DNS behavior](docs/deployment/DNS.md) |
| Releases and upgrades | [Packaging](docs/deployment/PACKAGING.md) · [Version compatibility](docs/deployment/VERSION-COMPATIBILITY.md) |
| Routine operations | [Runbook](docs/deployment/RUNBOOK.md) · [Backup and recovery](docs/deployment/BACKUP.md) |
| Monitoring | [Metrics](docs/deployment/METRICS.md) · [Monitoring and alerts](docs/deployment/MONITORING-ALERTS.md) |
| Protocol and design | [Protocol contract](docs/PROTOCOL.md) · [Architecture](docs/ARCHITECTURE.md) · [Project positioning](docs/POSITIONING.md) |

## Build From Source

```bash
cargo build --release
cargo test --workspace --all-targets
```

Main quality gates:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```

## Project Layout

```text
crates/espejismo-core     Shared protocol, crypto, config, admin, mux, transport
crates/espejismo-client   espejismo-local
crates/espejismo-server   espejismo-remote
configs/examples          One-file TOML example
docs/deployment           Configuration and operations docs
scripts                   Thin release download installers only
```

## Responsible Use

Use Espejismo only for systems and networks you own or are explicitly
authorized to administer. Traffic shaping can reduce some stable fingerprints,
but it does not make endpoint IPs, timing, uptime, traffic volume, or deployment
mistakes invisible.
