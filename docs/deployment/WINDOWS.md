# Windows Deployment

Espejismo runs as ordinary Windows console processes. The release installer
downloads and extracts the binaries and example config; it does not register a
Windows service, add firewall rules, change proxy settings, or start a process
in the background. This keeps the deployment explicit: start the client or
server yourself, or use Windows tools if you need automatic startup.

## Install and configure

Run this in PowerShell to install the full package for the current Windows
architecture under `%LOCALAPPDATA%\Espejismo`:

```powershell
iwr -useb https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.ps1 | iex
Copy-Item "$env:LOCALAPPDATA\Espejismo\configs\espejismo.toml" .\espejismo.toml
```

Edit `.espejismo.toml` with the correct PSK and server address, then validate
and probe the server before starting the client:

```powershell
$bin = "$env:LOCALAPPDATA\Espejismo\bin"
& "$bin\espejismo-local.exe" --config .\espejismo.toml --check-config
& "$bin\espejismo-local.exe" --config .\espejismo.toml --probe-server
& "$bin\espejismo-local.exe" --config .\espejismo.toml
```

Keep that terminal open while using the proxy. Configure applications to use
the configured SOCKS5 or HTTP listener, normally `127.0.0.1:6680` or
`127.0.0.1:6681`. For a remote Windows server, start
`espejismo-remote.exe --config .\espejismo.toml` in a PowerShell window.

The installer defaults to a per-user directory. To choose another location,
set `$env:ESPEJISMO_INSTALL_DIR` before running `install.ps1`. The
configuration contains tunnel credentials: keep it in a user-controlled
directory and restrict read access if other local accounts are untrusted.
The installer does not add the binaries to `PATH`; invoke them by full path or
from their `bin` directory.

## Firewall and background operation

The client normally needs outbound TCP access to the server's configured
`local.server` endpoint. On a Windows server, allow inbound TCP on the port
configured by `remote.listen` (the example is `6690`) in Windows Defender
Firewall and any cloud firewall/security group. The local SOCKS5 and HTTP
listeners should remain bound to loopback unless you intentionally want other
machines to use them and have protected the network accordingly.

For unattended operation, use Windows Task Scheduler or a service wrapper that
you administer. Configure the executable, `--config` argument, working
directory, account, and restart policy explicitly. Ordinary proxy mode does
not need elevation; TUN route or DNS changes do. Espejismo does not provide a
Windows service manager or silently install one.

## Optional TUN mode

TUN is optional; use SOCKS5/HTTP mode if you only need selected applications
to use the tunnel. For system-level IPv4 capture, open PowerShell as
Administrator and enable TUN route/DNS takeover explicitly:

```powershell
& "$env:LOCALAPPDATA\Espejismo\bin\espejismo-local.exe" `
  --config .\espejismo.toml `
  --tun-enabled --tun-auto-route --tun-auto-dns --check-config
```

After reviewing diagnostics, run the same command without `--check-config`.
Windows automatic DNS takeover currently manages IPv4 DNS only; specify IPv4
DNS servers such as `1.1.1.1`. Windows release packages include the matching
`wintun.dll` alongside the binaries. Keep the release package's `bin` directory
intact so the DLL remains available to the process.

Normal Ctrl-C shutdown attempts to restore routes and DNS. If the process or
machine stops before cleanup, reopen an elevated PowerShell and run:

```powershell
& "$env:LOCALAPPDATA\Espejismo\bin\espejismo-local.exe" `
  --config .\espejismo.toml --tun-route-cleanup
```

See [TUN mode](TUN.md) for routing behavior, limitations, and configuration.
