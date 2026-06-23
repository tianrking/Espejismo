# Espejismo

[Español](README_ES.md) | [Configuration](docs/deployment/CONFIG.md) | [TUN Mode](docs/deployment/TUN.md) | [Protocol](docs/PROTOCOL.md)

![Release](https://img.shields.io/badge/release-v0.1.0-0b7285)
![Rust](https://img.shields.io/badge/rust-native-9a3412)
![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20macOS%20%7C%20windows-1f6feb)
![Ingress](https://img.shields.io/badge/ingress-socks5%20%7C%20http%20%7C%20tun-2f9e44)
![License](https://img.shields.io/badge/license-MIT-495057)

Espejismo is a native Rust encrypted tunnel for running private client traffic
through an authenticated remote egress server. It keeps the operational model
small: one server binary, one local client binary, one TOML configuration file,
and release archives that can be installed with a single command.

## Technical Profile

| Layer | What ships in `v0.1.0` |
| --- | --- |
| Client ingress | SOCKS5, HTTP proxy, and native TUN capture |
| Remote egress | Authenticated TCP listener with configurable outbound policy |
| Transport | TCP underlay with encrypted framed streams and `yamux` multiplexing |
| Cryptography | X25519 session setup and XChaCha20-Poly1305 protected frames |
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
| `ESPEJISMO_VERSION` | `latest` | Release tag such as `v0.1.0` |
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

| Topic | Link |
| --- | --- |
| Complete configuration reference | [docs/deployment/CONFIG.md](docs/deployment/CONFIG.md) |
| Quick deployment path | [docs/deployment/QUICKSTART.md](docs/deployment/QUICKSTART.md) |
| Native TUN mode | [docs/deployment/TUN.md](docs/deployment/TUN.md) |
| CLI flags | [docs/deployment/CLI.md](docs/deployment/CLI.md) |
| Packaging and release artifacts | [docs/deployment/PACKAGING.md](docs/deployment/PACKAGING.md) |
| Protocol contract | [docs/PROTOCOL.md](docs/PROTOCOL.md) |

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
