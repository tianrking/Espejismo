# Espejismo

Espejismo is a native Rust encrypted tunnel for public and untrusted networks.
It ships two binaries:

- `espejismo-remote`: remote authenticated egress endpoint.
- `espejismo-local`: local SOCKS5, HTTP proxy, and optional TUN ingress.

The stable production path is TCP underlay, X25519 session establishment,
XChaCha20-Poly1305 encrypted frames, and `yamux` logical stream multiplexing.
The in-tree native mux and UDP physical underlay primitives remain experimental
or beta paths.

## Install From Release

Linux/macOS, or Windows Git Bash:

```bash
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh | sh
```

Windows PowerShell:

```powershell
iwr -useb https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.ps1 | iex
```

Useful installer variables:

```bash
ESPEJISMO_VERSION=latest          # or v0.0.9
ESPEJISMO_PACKAGE=full            # full or server
ESPEJISMO_INSTALL_DIR=$HOME/.espejismo
ESPEJISMO_REPO=tianrking/Espejismo
ESPEJISMO_ARCHIVE_URL=https://example.com/espejismo-linux-amd64.tar.gz
```

The installer only downloads and extracts the matching GitHub Release package.
It does not create services, firewall rules, or hidden background processes.

## One Config File

Use `configs/examples/espejismo.toml` as the single config shape for both sides.
The server reads `[shared]`, `[remote]`, `[logging]`, and `[admin]`. The client
reads `[shared]`, `[local]`, `[logging]`, and `[admin]`.

Minimum server/client edit:

```toml
[shared]
psk = "change-me-to-a-long-random-secret"

[local]
server = "YOUR_SERVER_IP_OR_DOMAIN:6690"
socks5_listen = "127.0.0.1:6680"
http_listen = "127.0.0.1:6681"

[remote]
listen = "0.0.0.0:6690"
```

Run server:

```bash
~/.espejismo/bin/espejismo-remote --config ~/.espejismo/configs/espejismo.toml
```

Run client:

```bash
~/.espejismo/bin/espejismo-local --config ~/.espejismo/configs/espejismo.toml
```

Then point applications at:

```text
SOCKS5: 127.0.0.1:6680
HTTP:   127.0.0.1:6681
```

Full parameter documentation lives in
[`docs/deployment/CONFIG.md`](docs/deployment/CONFIG.md).

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
