# Packaging

Espejismo is distributed as two native binaries plus configuration:

- `espejismo-local`: local SOCKS5/HTTP proxy.
- `espejismo-remote`: remote tunnel endpoint.
- `configs/espejismo.toml`: starter shared configuration.
- `README.md`, `README_ES.md`, and `CHANGELOG.md`: top-level usage and
  release notes.
- `scripts/setup-windows.ps1`: Windows config generator and launcher.
- `scripts/install-ubuntu-remote.sh`: Ubuntu remote installer for one-line
  server deployment.

## GitHub Release Artifacts

The release workflow builds these packages:

| Package | Runner | Rust target |
| --- | --- | --- |
| `espejismo-linux-amd64.tar.gz` | Ubuntu | `x86_64-unknown-linux-gnu` |
| `espejismo-linux-386.tar.gz` | Ubuntu + cross | `i686-unknown-linux-gnu` |
| `espejismo-linux-arm64.tar.gz` | Ubuntu + cross | `aarch64-unknown-linux-gnu` |
| `espejismo-linux-armv7.tar.gz` | Ubuntu + cross | `armv7-unknown-linux-gnueabihf` |
| `espejismo-darwin-arm64.tar.gz` | macOS Apple Silicon | `aarch64-apple-darwin` |
| `espejismo-windows-amd64.zip` | Windows | `x86_64-pc-windows-msvc` |
| `espejismo-windows-386.zip` | Windows | `i686-pc-windows-msvc` |
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
binaries, starter config, English and Spanish READMEs, changelog, architecture
notes, CLI reference, admin guide, egress guide, logging guide, packaging guide,
profile guide, users guide, update guide, status, test plan, and deployment
helper scripts.

## Publishing v0.0.5

After the version bump and verification commit is merged, create and push the
release tag:

```bash
git tag v0.0.5
git push origin v0.0.5
```

The GitHub release workflow runs on `v*` tags and publishes all platform
archives to the GitHub release.

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

The binaries can produce and decode that one-line config form directly:

```bash
CONFIG_B64="$(./bin/espejismo-local --config configs/espejismo.toml --print-config-base64)"
./bin/espejismo-local --decode-config-base64 "$CONFIG_B64" > configs/espejismo.toml
```

PowerShell base64:

```powershell
$CONFIG_B64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes((Get-Content .\configs\espejismo.toml -Raw)))
.\bin\espejismo-remote.exe --config-base64 $CONFIG_B64
.\bin\espejismo-local.exe --config-base64 $CONFIG_B64
```

PowerShell direct conversion:

```powershell
$CONFIG_B64 = .\bin\espejismo-local.exe --config .\configs\espejismo.toml --print-config-base64
.\bin\espejismo-local.exe --decode-config-base64 $CONFIG_B64 > .\configs\espejismo.toml
```
