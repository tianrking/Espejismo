# Packaging

Espejismo is distributed as two native binaries plus configuration:

- `espejismo-local`: local SOCKS5/HTTP proxy.
- `espejismo-remote`: remote tunnel endpoint.
- `wintun.dll` (Windows archives only): runtime required for Windows TUN mode.
- `configs/espejismo.toml`: starter shared configuration.
- `README.md`, `README_ES.md`, and `CHANGELOG.md`: top-level usage and
  release notes.
- `scripts/install.sh`: guided Linux/macOS installer and manager bootstrap.
- `scripts/install.ps1`: guided Windows installer and manager bootstrap.
- `scripts/setup-windows.ps1`: Windows config generator and launcher.
- `scripts/install-ubuntu-remote.sh`: Ubuntu remote installer for one-line
  server deployment. It installs only `espejismo-remote` plus an
  `espejismoctl` manager on the server.

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

For server one-line installs, the workflow also publishes server-only packages
with the same platform names and an `espejismo-server-` prefix, for example
`espejismo-server-linux-amd64.tar.gz`. Those packages contain only
`bin/espejismo-remote`, the server install helper, and server deployment docs.

Windows archives include `bin/wintun.dll`, so users can run TUN mode without a
separate manual Wintun download.

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

## Publishing v0.0.9

After the version bump and verification commit is merged, create or update the
release tag on the exact commit that should be packaged:

```bash
git tag -f -a v0.0.9 -m "v0.0.9"
git push origin main
git push --force origin v0.0.9
```

The GitHub release workflow runs on `v*` tags and publishes all platform
archives to a non-draft GitHub release. If a draft release already exists for
the tag, the rerun updates that release with assets built from the new tag
target.

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
