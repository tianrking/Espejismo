# Espejismo

**[English](README.md) | [Español](README_ES.md)**

<p>
  <img src="https://img.shields.io/badge/Rust-1.75%2B-dea584?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Tokio-async_runtime-ff5e00?logo=tokio&logoColor=white" alt="Tokio">
  <img src="https://img.shields.io/badge/XChaCha20--Poly1305-AEAD-4a90d9" alt="AEAD">
  <img src="https://img.shields.io/badge/X25519-key__exchange-e97326" alt="X25519">
  <img src="https://img.shields.io/badge/License-MIT-blue" alt="License">
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey" alt="Platform">
</p>

Espejismo is a native Rust encrypted transport tunnel for public and untrusted
networks. It provides a local SOCKS5/HTTP proxy, optional native TUN ingress,
X25519 session establishment, XChaCha20-Poly1305 encrypted frames, TCP-friendly
pacing, adaptive frame shaping, and selectable logical stream multiplexing.

Current release target: `v0.0.7`.

## Quick Start

User installation, one-line scripts, service management, config files, and
manual startup commands live in the deployment guide:

**[Open the Quickstart](docs/deployment/QUICKSTART.md)**

The shortest guided install paths are:

```bash
# Linux/macOS guided installer.
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh | bash

# Linux remote server, non-interactive root install.
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh | sudo bash

# Linux remote server with an explicit public domain or IP.
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh \
  | sudo ESPEJISMO_ROLE=remote ESPEJISMO_PUBLIC_HOST=proxy.example.com bash

# Linux local client, explicit non-interactive install.
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh \
  | ESPEJISMO_ROLE=local ESPEJISMO_SERVER=203.0.113.10:6690 bash
```

```powershell
# Windows guided installer.
iwr -useb https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.ps1 | iex
```

The installers download the matching binary from GitHub Releases latest,
generate random secrets by default, write config, start the selected local or
remote role, and print management plus connection commands. After install, run
`~/.espejismo/espejismoctl connect` to see the SOCKS5/HTTP proxy address,
curl test commands, or the remote client import profile.

In non-interactive mode, root Linux defaults to `remote`; non-root Linux/macOS
defaults to `local`. Set `ESPEJISMO_ROLE=local` or `ESPEJISMO_ROLE=remote` to be
explicit.

Remote installs auto-detect the public IP when no endpoint is provided.
Use `ESPEJISMO_PUBLIC_HOST=your.domain` or
`ESPEJISMO_PUBLIC_ENDPOINT=your.domain:6690` when you already know the client
dial address. `0.0.0.0` is only for `ESPEJISMO_LISTEN`, not for public client
profiles.

Local SOCKS5/HTTP proxy authentication is disabled by default because listeners
bind to `127.0.0.1`. Set `ESPEJISMO_LOCAL_AUTH_PASSWORD` when you explicitly
want browser/app proxy authentication.

Installer-written configs are role-specific: remote installs keep client-only
`local` settings out of the server config and generate them only in the printed
client import profile.

Configs and client profiles are reversible for the local onboarding path:

```bash
espejismo-local --config client.toml --print-client-profile --profile-name laptop
espejismo-local --import-profile "espejismo://import/..." --write-config client.toml
```

## Release Downloads

Normal users do not need Rust or Cargo. Download the latest package for your
platform from GitHub Releases:

- `espejismo-linux-amd64.tar.gz`
- `espejismo-linux-386.tar.gz`
- `espejismo-linux-arm64.tar.gz`
- `espejismo-linux-armv7.tar.gz`
- `espejismo-darwin-arm64.tar.gz`
- `espejismo-windows-amd64.zip`
- `espejismo-windows-386.zip`
- `espejismo-windows-arm64.zip`

Each release package contains:

- `bin/espejismo-local`
- `bin/espejismo-remote`
- `configs/espejismo.toml`
- `scripts/install.sh`
- `scripts/install.ps1`
- deployment and testing documentation

## Project Layout

```text
crates/espejismo-core     Shared protocol, crypto, config, admin, mux, transport
crates/espejismo-client   espejismo-local: SOCKS5/HTTP/TUN local ingress
crates/espejismo-server   espejismo-remote: authenticated remote egress
configs/examples          Starter TOML config
docs/deployment           User deployment and operations docs
docs/development          Status and engineering notes
scripts                   Installers, packaging, smoke, stress, benchmark helpers
```

## Developer Build

Developers who clone the repository need Rust/Cargo. Normal users should use the
release packages or guided installers from the Quickstart.

```bash
git clone https://github.com/tianrking/Espejismo.git
cd Espejismo
cargo build --release
```

Run the binaries from source:

```bash
ESPEJISMO_PSK='change-me-long-random-secret' \
cargo run --bin espejismo-remote -- --listen 0.0.0.0:6690

ESPEJISMO_PSK='change-me-long-random-secret' \
cargo run --bin espejismo-local -- \
  --server 127.0.0.1:6690 \
  --socks5-listen 127.0.0.1:6680 \
  --http-listen 127.0.0.1:6681
```

Generate and validate config:

```bash
cargo run --bin espejismo-local -- --print-example-config > espejismo.toml
cargo run --bin espejismo-local -- --config espejismo.toml --check-config
cargo run --bin espejismo-remote -- --config espejismo.toml --check-config
```

Run the main checks:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
./scripts/e2e_smoke.sh
REQUESTS=200 CONCURRENCY=32 ./scripts/stress_smoke.sh
OUT=target/mux-benchmark.json ./scripts/benchmark_mux.sh
```

Build a local release package:

```bash
./scripts/package-release.sh
```

Windows:

```powershell
.\scripts\package-release.ps1
```

## Documentation

- [Quickstart](docs/deployment/QUICKSTART.md)
- [CLI Reference](docs/deployment/CLI.md)
- [Admin And Metrics](docs/deployment/ADMIN.md)
- [Profiles](docs/deployment/PROFILES.md)
- [Users, Quotas, Bandwidth](docs/deployment/USERS.md)
- [Native TUN Mode](docs/deployment/TUN.md)
- [Packaging](docs/deployment/PACKAGING.md)
- [Protocol Specification](docs/PROTOCOL.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Implementation Status](docs/development/STATUS.md)
- [Test Plan](docs/testing/TEST_PLAN.md)
- [Design Principles](docs/research/DESIGN_PRINCIPLES.md)

## Responsible Use

Espejismo is intended for encrypted access to systems you own or are explicitly
authorized to administer, such as a private server, lab, or internal test
environment. It is not a service, anonymity network, or authorization bypass
tool.

Traffic shaping can reduce some protocol fingerprints, but it does not make a
connection invisible. Operators should assume that endpoints, timing, byte
volume, uptime, routing path, and deployment mistakes may still be observable.

You are responsible for complying with all applicable laws, network policies,
terms of service, export controls, and authorization boundaries in your
jurisdiction and in any network where you deploy or use this software.
