# Packaging

Espejismo is distributed as two native binaries plus configuration:

- `espejismo-local`: local SOCKS5/HTTP proxy.
- `espejismo-remote`: remote tunnel endpoint.
- `configs/espejismo.toml`: starter shared configuration.
- `scripts/setup-windows.ps1`: Windows config generator and launcher.
- `scripts/install-ubuntu-remote.sh`: Ubuntu remote installer for one-line
  server deployment.

## GitHub Release Artifacts

The release workflow builds these packages:

| Package | Runner | Rust target |
| --- | --- | --- |
| `espejismo-linux-x86_64.tar.gz` | Ubuntu | `x86_64-unknown-linux-gnu` |
| `espejismo-linux-x86.tar.gz` | Ubuntu + cross | `i686-unknown-linux-gnu` |
| `espejismo-linux-arm64.tar.gz` | Ubuntu + cross | `aarch64-unknown-linux-gnu` |
| `espejismo-linux-arm32.tar.gz` | Ubuntu + cross | `armv7-unknown-linux-gnueabihf` |
| `espejismo-macos-x86_64.tar.gz` | macOS Intel | `x86_64-apple-darwin` |
| `espejismo-macos-aarch64.tar.gz` | macOS Apple Silicon | `aarch64-apple-darwin` |
| `espejismo-windows-x86_64.zip` | Windows | `x86_64-pc-windows-msvc` |
| `espejismo-windows-x86.zip` | Windows | `i686-pc-windows-msvc` |
| `espejismo-windows-arm64.zip` | Windows | `aarch64-pc-windows-msvc` |

The workflow runs on `v*` tags and can also be started manually from GitHub
Actions.

## Local Packaging

Unix-like hosts:

```bash
./scripts/package-release.sh
```

Windows PowerShell:

```powershell
.\scripts\package-release.ps1
```

Both scripts build release binaries and create a `dist/` archive containing the
binaries, starter config, README, architecture notes, admin guide, egress guide,
logging guide, packaging guide, profile guide, status, test plan, and deployment
helper scripts.

## Configuration Import

The same TOML config can be supplied as a file:

```bash
./bin/espejismo-remote --config configs/espejismo.toml
./bin/espejismo-local --config configs/espejismo.toml
```

or as a base64 one-line import:

```bash
CONFIG_B64="$(base64 -w0 configs/espejismo.toml)"
./bin/espejismo-remote --config-base64 "$CONFIG_B64"
./bin/espejismo-local --config-base64 "$CONFIG_B64"
```

PowerShell base64:

```powershell
$CONFIG_B64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes((Get-Content .\configs\espejismo.toml -Raw)))
.\bin\espejismo-remote.exe --config-base64 $CONFIG_B64
.\bin\espejismo-local.exe --config-base64 $CONFIG_B64
```
