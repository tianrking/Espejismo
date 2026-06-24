# Packaging

Release artifacts are built by `.github/workflows/release.yml`.

Full packages are named:

```text
espejismo-<platform>-<arch>.tar.gz
espejismo-windows-<arch>.zip
```

Server-only packages are named:

```text
espejismo-server-<platform>-<arch>.tar.gz
espejismo-server-windows-<arch>.zip
```

Each full package contains:

```text
bin/espejismo-local
bin/espejismo-remote
bin/espejismo-bench-http
configs/espejismo.toml
docs/
scripts/install.sh
scripts/install.ps1
```

Windows full packages also include `bin/wintun.dll`.

Supported release suffixes:

```text
linux-amd64
linux-386
linux-arm64
linux-armv7
darwin-arm64
windows-amd64
windows-386
windows-arm64
```

Local packaging scripts were intentionally removed. Use GitHub Actions for
official cross-platform release artifacts, or run the equivalent build command
locally for the current platform:

```bash
cargo build --release --locked --bin espejismo-local --bin espejismo-remote --bin espejismo-bench-http
```

The download installers in `scripts/` are deliberately thin and only fetch
published release artifacts.
