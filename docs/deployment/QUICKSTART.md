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
  | sudo ESPEJISMO_REPO=OWNER/REPO ESPEJISMO_VERSION=v0.0.4 bash
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
ESPEJISMO_TCP_NODELAY=true
ESPEJISMO_TCP_KEEPALIVE_SECS=30
ESPEJISMO_TCP_HEARTBEAT_SECS=30
ESPEJISMO_TCP_USER_TIMEOUT_MS=30000
ESPEJISMO_MUX_MODE=yamux
ESPEJISMO_NATIVE_MUX_INITIAL_WINDOW_BYTES=1048576
ESPEJISMO_NATIVE_MUX_STREAM_BUFFER_FRAMES=128
ESPEJISMO_NATIVE_MUX_IDLE_TIMEOUT_SECS=300
ESPEJISMO_PACING_ENABLED=true
ESPEJISMO_PACING_MAX_BYTES_PER_SEC=0
ESPEJISMO_PACING_BURST_BYTES=65536
ESPEJISMO_PACING_MIN_WRITE_BYTES=1024
ESPEJISMO_OBFUSCATION_PROFILE=balanced
ESPEJISMO_RANDOMIZE_CHUNKS=true
ESPEJISMO_MIN_CHUNK=1024
ESPEJISMO_MAX_CHUNK=16384
ESPEJISMO_STEALTH_FRAME_SIZE=4096
ESPEJISMO_STEALTH_TICK_MS=50
ESPEJISMO_CLIENT_TUNNEL_POOL_MIN_CONNECTIONS=1
ESPEJISMO_CLIENT_TUNNEL_POOL_MAX_CONNECTIONS=4
ESPEJISMO_CLIENT_TUNNEL_POOL_INTERACTIVE_LANES=1
ESPEJISMO_CLIENT_TUNNEL_POOL_BULK_LANES=2
ESPEJISMO_CLIENT_TUNNEL_POOL_MAX_RECONNECT_ATTEMPTS=3
ESPEJISMO_OPEN_UFW=1
```

Inspect the service:

```bash
systemctl status espejismo-remote --no-pager
journalctl -u espejismo-remote -f
```

## Direct Binary Start

Downloaded release archives can run without Rust or Cargo.

Remote server, Linux/macOS:

```bash
ESPEJISMO_PSK='change-me-long-random-secret' \
./bin/espejismo-remote --listen 0.0.0.0:6690
```

Local client, Linux/macOS:

```bash
ESPEJISMO_PSK='change-me-long-random-secret' \
./bin/espejismo-local \
  --server remote.example.com:6690 \
  --socks5-listen 127.0.0.1:6680 \
  --http-listen 127.0.0.1:6681
```

Remote server, Windows PowerShell:

```powershell
$env:ESPEJISMO_PSK = "change-me-long-random-secret"
.\bin\espejismo-remote.exe --listen 0.0.0.0:6690
```

Local client, Windows PowerShell:

```powershell
$env:ESPEJISMO_PSK = "change-me-long-random-secret"
.\bin\espejismo-local.exe --server remote.example.com:6690 --socks5-listen 127.0.0.1:6680 --http-listen 127.0.0.1:6681
```

For production deployments, use `--config`, `--config-base64`, or
`espejismo://import/...` profiles so secrets are not kept in shell history.

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
- `shared.max_streams`: concurrent logical stream limit per physical tunnel.
- `shared.idle_timeout_secs`: idle stream timeout.
- `shared.mux.mode`: logical stream multiplexer. Keep `yamux` for production
  stability; use `native` for the in-tree alpha mux test path.
- `shared.mux.native_initial_window_bytes`: native mux send window per stream.
- `shared.mux.native_stream_buffer_frames`: native mux bounded receive queue per
  stream.
- `shared.mux.native_idle_timeout_secs`: idle native mux session timeout before
  GOAWAY.
- `local.tunnel_pool.max_reconnect_attempts`: per-request reconnect attempts
  before returning an explicit local proxy error.
- `shared.obfuscation.profile`: sender-side traffic shape. Use `low_latency`,
  `balanced`, `high_entropy`, `bulk`, or `stealth`.
- `shared.obfuscation.chunk_policy`: adaptive data chunk sizing. Use
  `low_latency` for 2-8 KiB chunks, `balanced` for 4-16 KiB chunks, `bulk` for
  16-64 KiB chunks, `stealth` for fixed stealth capacity, or `custom` to honor
  `min_chunk` / `max_chunk`.
- `shared.stealth.frame_size` / `shared.stealth.tick_ms`: fixed frame size and
  base pacing when `profile = "stealth"`. The transport starts with a short
  random padding warmup, sends data or padding on a paced cadence, and slows
  idle padding toward heartbeat-like intervals.
- `local.server`: remote server address.
- `local.tunnel_pool`: number of physical TCP tunnel lanes and their
  interactive/bulk split. New streams are assigned by priority and lane health.
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
