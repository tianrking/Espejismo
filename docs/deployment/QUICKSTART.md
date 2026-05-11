# Deployment Quickstart

This guide covers the normal production shape:

1. Install `espejismo-remote` on an Ubuntu server.
2. Run `espejismo-local` on your own machine.
3. Point local applications at `127.0.0.1:6680` for SOCKS5 or
   `127.0.0.1:6681` for HTTP proxy.

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
    ESPEJISMO_PUBLIC_ENDPOINT=203.0.113.10:6690 \
    bash
```

Pinned release:

```bash
curl -fsSL https://raw.githubusercontent.com/OWNER/REPO/main/scripts/install-ubuntu-remote.sh \
  | sudo ESPEJISMO_REPO=OWNER/REPO ESPEJISMO_VERSION=v0.0.2 bash
```

Custom archive URL, useful for private releases or self-hosted packages:

```bash
curl -fsSL https://example.com/install-ubuntu-remote.sh \
  | sudo ESPEJISMO_ARCHIVE_URL=https://example.com/espejismo-linux-amd64.tar.gz bash
```

The installer downloads `espejismo-linux-amd64.tar.gz`, installs
`espejismo-remote` and `espejismo-local` into `/usr/local/bin`, writes
`/etc/espejismo/espejismo.toml`, creates an `espejismo` system user, installs a
systemd unit, and starts `espejismo-remote`.

Useful install-time variables:

```bash
ESPEJISMO_LISTEN=0.0.0.0:6690
ESPEJISMO_PUBLIC_ENDPOINT=203.0.113.10:6690
# Or use ESPEJISMO_PUBLIC_HOST=203.0.113.10 with the port from ESPEJISMO_LISTEN.
ESPEJISMO_PSK='use-a-long-random-secret'
ESPEJISMO_CLIENT_SOCKS5_LISTEN=127.0.0.1:6680
ESPEJISMO_CLIENT_HTTP_LISTEN=127.0.0.1:6681
ESPEJISMO_CLIENT_AUTH_USER=local-user
ESPEJISMO_CLIENT_AUTH_PASSWORD='use-a-local-proxy-password'
ESPEJISMO_ADMIN_LISTEN=127.0.0.1:9090
ESPEJISMO_ADMIN_TOKEN='use-a-long-random-admin-token'
ESPEJISMO_DENY_PRIVATE_IPS=true
ESPEJISMO_ALLOW_PORTS=80,443
ESPEJISMO_BLOCK_PORTS=25
ESPEJISMO_MAX_STREAMS=256
ESPEJISMO_IDLE_TIMEOUT_SECS=300
ESPEJISMO_OBFUSCATION_PROFILE=balanced
ESPEJISMO_RANDOMIZE_CHUNKS=true
ESPEJISMO_MIN_CHUNK=1024
ESPEJISMO_MAX_CHUNK=16384
ESPEJISMO_STEALTH_FRAME_SIZE=4096
ESPEJISMO_STEALTH_TICK_MS=50
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
SOCKS5:     127.0.0.1:6680
HTTP proxy: 127.0.0.1:6681
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
  -RemoteListen "0.0.0.0:6690" `
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
- `shared.obfuscation.profile`: sender-side traffic shape. Use `low_latency`,
  `balanced`, `high_entropy`, or `stealth`.
- `shared.stealth.frame_size` / `shared.stealth.tick_ms`: fixed frame size and
  base pacing when `profile = "stealth"`. The transport starts with a short
  random padding warmup, sends data or padding on a paced cadence, and slows
  idle padding toward heartbeat-like intervals.
- `local.server`: remote server address.
- `local.socks5_listen`: local SOCKS5 listener.
- `local.http_listen`: local HTTP proxy listener.
- `remote.listen`: remote public listener.
- `remote.fallback_http.mode`: use `silent` for quiet handling, or
  `http_fallback` to route common HTTP probe prefixes to fallback.
- `remote.fallback_http.upstream`: optional local TCP endpoint (for example
  `127.0.0.1:8080`) that receives fallback probe traffic.
- `remote.egress.deny_private_ips`: block private, loopback, link-local, and
  special egress targets.
- `remote.egress.allow_ports` / `block_ports`: outbound port policy.
- `remote.users`: optional multi-user credentials. Each user can have an
  independent PSK, rolling byte quota, and aggregate bandwidth limit.
- `remote.egress.socks5_proxy`: optional no-auth SOCKS5 chain for TCP and UDP
  egress.

Config exchange:

```bash
espejismo-local --config espejismo.toml --print-config-base64
espejismo-local --decode-config-base64 "BASE64_CONFIG"
espejismo-local --config espejismo.toml --print-client-profile --profile-name laptop
espejismo-local --import-profile "espejismo://import/..."
```

Remote runtime apply:

```bash
curl -X POST -H "Authorization: Bearer $ESPEJISMO_ADMIN_TOKEN" \
  --data-binary @/etc/espejismo/espejismo.toml \
  http://127.0.0.1:9090/apply
```

The generated profile/config contains secret material. Do not paste it into
logs, issue trackers, chat, or shell history on shared systems.
