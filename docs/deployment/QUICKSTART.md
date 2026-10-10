# Deployment Quickstart

This quickstart uses release packages and one TOML config file. The installer
downloads and extracts binaries only; it does not create services or edit system
network settings.

For common questions about ports, connectivity, UDP, and protocol behavior, see
the [FAQ](FAQ.md).
For router port forwarding, CGNAT, and testing a NAT-hosted server, see
[NAT deployment](NAT.md).
For Windows-specific installation, firewall, and TUN notes, see
[Windows deployment](WINDOWS.md).

## Download

Linux/macOS, or Windows Git Bash:

```bash
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh | sh
```

Windows PowerShell:

```powershell
iwr -useb https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.ps1 | iex
```

Server-only package:

```bash
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh \
  | ESPEJISMO_PACKAGE=server sh
```

Pinned release:

```bash
curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh \
  | ESPEJISMO_VERSION=v0.1.5 sh
```

## Configure

Edit the extracted example:

```bash
cp ~/.espejismo/configs/espejismo.toml ./espejismo.toml
```

In the copied file, set the same long random secret in both
`[shared].psk` and `[[remote.users]].psk`. The example config already defines a
`default` remote user; changing only `[shared].psk` leaves the server expecting
the old user PSK, so the client handshake will fail. Keep the two values
identical on the server and client. Also set `[local].server` on the client to
the server's public IP or hostname and port. The server listens on
`[remote].listen`:

```toml
[shared]
psk = "change-me-to-a-long-random-secret"

[local]
server = "YOUR_SERVER_IP_OR_DOMAIN:6690"
socks5_listen = "127.0.0.1:6680"
http_listen = "127.0.0.1:6681"

[remote]
listen = "0.0.0.0:6690"

[[remote.users]]
name = "default"
psk = "same-long-random-secret-as-shared-psk"
```

The block above shows the relevant server authentication fields; retain the
other sections from the example config. On the client, configure `local.server`
as `"YOUR_SERVER_IP_OR_DOMAIN:6690"`. For a smaller config without per-user
authentication, remove the `[[remote.users]]` table on the server; then the
remote authenticates directly with `[shared].psk`.

Recommended remote egress guard:

```toml
[remote.egress]
deny_private_ips = true
allow_ports = [80, 443]
```

Full parameter reference: [CONFIG.md](CONFIG.md).

## Run Server

```bash
~/.espejismo/bin/espejismo-remote --config ./espejismo.toml
```

Windows:

```powershell
& "$env:LOCALAPPDATA\Espejismo\bin\espejismo-remote.exe" --config .\espejismo.toml
```

## Run Client

```bash
~/.espejismo/bin/espejismo-local --config ./espejismo.toml
```

Windows:

```powershell
& "$env:LOCALAPPDATA\Espejismo\bin\espejismo-local.exe" --config .\espejismo.toml
```

Point applications at:

```text
SOCKS5: 127.0.0.1:6680
HTTP:   127.0.0.1:6681
```

## Validate

```bash
~/.espejismo/bin/espejismo-remote --config ./espejismo.toml --check-config
~/.espejismo/bin/espejismo-local --config ./espejismo.toml --check-config
~/.espejismo/bin/espejismo-local --config ./espejismo.toml --probe-server
```

## Admin

Bind admin to loopback unless you have a strong reason to expose it:

```toml
[admin]
listen = "127.0.0.1:9090"
token = "change-me-admin-token"
```

Public admin listeners require `admin.token`.

```bash
curl -H "Authorization: Bearer change-me-admin-token" http://127.0.0.1:9090/status
```
