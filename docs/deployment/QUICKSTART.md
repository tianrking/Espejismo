# Deployment Quickstart

This guide covers the normal production shape:

1. Install `espejismo-remote` on an Ubuntu server.
2. Run `espejismo-local` on your own machine.
3. Point local applications at `127.0.0.1:1080` for SOCKS5 or
   `127.0.0.1:8080` for HTTP proxy.

## Ubuntu Remote One-Liner

After publishing a GitHub release, install the remote endpoint with one command.
Replace `OWNER/REPO` with the repository that hosts the release artifacts:

```bash
curl -fsSL https://raw.githubusercontent.com/OWNER/REPO/main/scripts/install-ubuntu-remote.sh \
  | sudo ESPEJISMO_REPO=OWNER/REPO ESPEJISMO_VERSION=latest bash
```

The installer prints a single `espejismo://import/...` client profile. Keep it
private. It contains the remote address, PSK, local proxy listeners, and local
proxy credentials.

For a ready-to-use client profile, pass the public endpoint that clients should
connect to:

```bash
curl -fsSL https://raw.githubusercontent.com/OWNER/REPO/main/scripts/install-ubuntu-remote.sh \
  | sudo ESPEJISMO_REPO=OWNER/REPO \
    ESPEJISMO_VERSION=latest \
    ESPEJISMO_PUBLIC_ENDPOINT=203.0.113.10:8443 \
    bash
```

Pinned release:

```bash
curl -fsSL https://raw.githubusercontent.com/OWNER/REPO/main/scripts/install-ubuntu-remote.sh \
  | sudo ESPEJISMO_REPO=OWNER/REPO ESPEJISMO_VERSION=v0.1.0 bash
```

Custom archive URL, useful for private releases or self-hosted packages:

```bash
curl -fsSL https://example.com/install-ubuntu-remote.sh \
  | sudo ESPEJISMO_ARCHIVE_URL=https://example.com/espejismo-linux-x86_64.tar.gz bash
```

The installer downloads `espejismo-linux-x86_64.tar.gz`, installs
`espejismo-remote` and `espejismo-local` into `/usr/local/bin`, writes
`/etc/espejismo/espejismo.toml`, creates an `espejismo` system user, installs a
systemd unit, and starts `espejismo-remote`.

Useful install-time variables:

```bash
ESPEJISMO_LISTEN=0.0.0.0:8443
ESPEJISMO_PUBLIC_ENDPOINT=203.0.113.10:8443
# Or use ESPEJISMO_PUBLIC_HOST=203.0.113.10 with the port from ESPEJISMO_LISTEN.
ESPEJISMO_PSK='use-a-long-random-secret'
ESPEJISMO_CLIENT_SOCKS5_LISTEN=127.0.0.1:1080
ESPEJISMO_CLIENT_HTTP_LISTEN=127.0.0.1:8080
ESPEJISMO_CLIENT_AUTH_USER=local-user
ESPEJISMO_CLIENT_AUTH_PASSWORD='use-a-local-proxy-password'
ESPEJISMO_ADMIN_LISTEN=127.0.0.1:9090
ESPEJISMO_ADMIN_TOKEN='use-a-long-random-admin-token'
ESPEJISMO_DENY_PRIVATE_IPS=true
ESPEJISMO_ALLOW_PORTS=80,443
ESPEJISMO_BLOCK_PORTS=25
ESPEJISMO_MAX_STREAMS=256
ESPEJISMO_IDLE_TIMEOUT_SECS=300
ESPEJISMO_OPEN_UFW=1
```

Inspect the service:

```bash
systemctl status espejismo-remote --no-pager
journalctl -u espejismo-remote -f
```

## Windows Local Client

Download the Windows release archive, extract it, and run the setup helper from
PowerShell:

```powershell
.\scripts\setup-windows.ps1 `
  -Mode local `
  -ProfileUrl "espejismo://import/..."
```

The script writes `configs\espejismo-local.toml` and starts
`bin\espejismo-local.exe`. Applications can then use:

```text
SOCKS5:     127.0.0.1:1080
HTTP proxy: 127.0.0.1:8080
```

Generate config without starting:

```powershell
.\scripts\setup-windows.ps1 `
  -Mode local `
  -ProfileUrl "espejismo://import/..." `
  -NoStart `
  -PrintCommand
```

## Windows Remote Server

Windows can also run the remote endpoint directly:

```powershell
.\scripts\setup-windows.ps1 `
  -Mode remote `
  -RemoteListen "0.0.0.0:8443" `
  -Psk "use-a-long-random-secret" `
  -AdminListen "127.0.0.1:9090"
```

Open the selected TCP listen port in Windows Firewall if the remote endpoint
must accept connections from other machines.

## Configuration Model

Both binaries read the same TOML shape. The remote uses `[shared]`,
`[logging]`, `[admin]`, and `[remote]`. The local client uses `[shared]`,
`[logging]`, `[admin]`, and `[local]`.

The most important knobs are:

- `shared.psk`: shared secret. Keep it private.
- `shared.max_streams`: concurrent yamux stream limit per physical tunnel.
- `shared.idle_timeout_secs`: idle stream timeout.
- `local.server`: remote server address.
- `local.socks5_listen`: local SOCKS5 listener.
- `local.http_listen`: local HTTP proxy listener.
- `remote.listen`: remote public listener.
- `remote.egress.deny_private_ips`: block private, loopback, link-local, and
  special egress targets.
- `remote.egress.allow_ports` / `block_ports`: outbound port policy.

The generated profile/config contains secret material. Do not paste it into
logs, issue trackers, chat, or shell history on shared systems.
