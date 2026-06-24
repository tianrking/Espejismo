# Native TUN Mode

TUN mode is an optional local ingress for system-level traffic capture. It is
disabled by default and does not change the stable SOCKS5/HTTP proxy path.

In ordinary mode, Espejismo is an explicit local proxy: only applications that
use the local SOCKS5 or HTTP proxy listeners enter the encrypted tunnel. In TUN
mode, Espejismo creates a virtual interface and can route ordinary IPv4 TCP/UDP
traffic from the operating system into the same encrypted protocol path, giving
the client a global-forwarding mode without changing the remote server.

## Support Matrix

| Capability | Linux | macOS | Windows | Notes |
|---|---|---|---|---|
| Ordinary proxy mode (SOCKS5/HTTP) | Yes | Yes | Yes | Explicit app-level proxy configuration. |
| TUN ingress (local capture) | Yes | Yes | Yes | Requires elevated privileges or platform entitlement. |
| Global IPv4 TCP forwarding via TUN | Yes | Yes | Yes | Split-default route takeover plus remote route protection. |
| Global IPv4 UDP forwarding via TUN | Yes | Yes | Yes | Application-level UDP relay over encrypted TCP mux tunnel. |
| Global IPv6 route takeover via TUN | No | No | No | Not advertised as complete in `v0.1.3`. |
| ICMP forwarding (`ping`) | No | No | No | Use TCP/HTTP probes for validation. |
| Physical UDP underlay takeover | No | No | No | UDP underlay remains reserved/experimental. |
| Auto DNS takeover | Yes | Yes | Yes (IPv4 DNS only) | Windows uses `netsh interface ipv4` DNS APIs. |
| Crash recovery via `--tun-route-cleanup` | Yes | Yes | Yes | Replays saved route/DNS state from temp file. |

```toml
[local.tun]
enabled = true
name = "esptun0"
address = "10.255.0.2"
prefix = 24
destination = "10.255.0.1"
mtu = 1500
udp_enabled = true
udp_timeout_secs = 3
udp_block_ports = [443]

[local.tun.route]
enabled = true
protect_server_route = true
dns_enabled = true
dns_servers = ["1.1.1.1", "8.8.8.8"]
```

Equivalent local CLI overrides:

```bash
sudo espejismo-local \
  --config espejismo.toml \
  --tun-enabled \
  --tun-name esptun0 \
  --tun-auto-route \
  --tun-auto-dns \
  --tun-dns 1.1.1.1,8.8.8.8 \
  --tun-udp-block-ports 443
```

Windows PowerShell equivalent:

```powershell
.\espejismo-local.exe `
  --config espejismo.toml `
  --tun-enabled `
  --tun-name esptun0 `
  --tun-auto-route `
  --tun-auto-dns `
  --tun-dns 1.1.1.1,8.8.8.8
```

macOS equivalent:

```bash
sudo ./espejismo-local \
  --config espejismo.toml \
  --tun-enabled \
  --tun-name utun123 \
  --tun-auto-route \
  --tun-auto-dns \
  --tun-dns 1.1.1.1,8.8.8.8
```

How it works:

- `espejismo-local` creates a native TUN interface.
- A userspace netstack converts TUN TCP flows into existing tunnel TCP CONNECT
  streams.
- UDP datagrams are converted into existing tunnel UDP relay requests.
- UDP relay is request/response oriented. UDP/443 is blocked by default to make
  browsers fall back from QUIC to TCP HTTPS, which avoids long-lived QUIC
  timeout bursts in global TUN mode. Set `udp_block_ports = []` if you need
  UDP/443 relay and accept the additional timeout risk.
- `espejismo-remote` is unchanged; existing users, quota, bandwidth, egress,
  logging, and admin status still apply.
- The current automatic route takeover is IPv4 split-default routing. IPv6
  global takeover is intentionally not advertised as complete in this release.
- ICMP echo is not proxied. Use TCP/HTTP probes instead of `ping`.

Important deployment notes:

- Creating a TUN interface usually requires root/admin privileges or a platform
  VPN entitlement. On Windows, run the local process from an elevated
  PowerShell or terminal.
- Linux route takeover is opt-in through `[local.tun.route].enabled = true` or
  `--tun-auto-route`.
- When Linux route takeover is enabled, Espejismo first resolves
  `local.server`, installs high-priority direct policy rules for the remote
  IPv4 addresses through the existing `main` table, then installs an Espejismo
  policy route table for ordinary traffic through the TUN device. It does not
  replace the `main` default route. This makes Linux TUN coexist more reliably
  with systems that already have policy routing rules.
- Before installing Linux route/DNS takeover, `espejismo-local` opens a small
  warm-up tunnel stream through the existing network path. This confirms that
  the physical tunnel is alive before DNS and application traffic are routed
  into TUN, avoiding startup deadlocks during the first DNS burst.
- Windows route takeover is also opt-in. It protects the remote server route
  through the current default gateway and installs split-default
  `0.0.0.0/1` plus `128.0.0.0/1` routes through the TUN interface, leaving the
  original default route intact for recovery. The automatic Windows DNS manager
  currently applies IPv4 DNS servers through `netsh interface ipv4`; use IPv4
  DNS addresses such as `1.1.1.1` and `8.8.8.8`.
- macOS route takeover is also opt-in. It protects the remote server route
  through the current default gateway and installs the same split-default
  `0.0.0.0/1` plus `128.0.0.0/1` routes through the TUN interface.
- DNS takeover is opt-in through `[local.tun.route].dns_enabled = true` or
  `--tun-auto-dns`. On Linux it uses `resolvectl dns`, `resolvectl domain ~.`,
  and `resolvectl default-route yes`. On Windows it uses `netsh interface ipv4`
  to assign DNS servers to the TUN interface and restores the previous DHCP or
  static DNS state on shutdown. On macOS it uses `networksetup` to save and
  apply DNS servers for network services, then restores the previous empty or
  static DNS state on shutdown.
- On Ctrl-C or SIGTERM, Espejismo reverts the TUN DNS settings and removes the
  policy routing rules/routes on a best-effort basis. Older recovery state files
  from previous versions that modified the `main` default route are still
  restored by `--tun-route-cleanup`.
- During route takeover Espejismo writes a small recovery state file under the
  OS temporary directory. If the process is killed before `Drop` can run, use
  `--tun-route-cleanup` to replay that state and remove the saved file.
- The current TUN implementation is intended for TCP-first deployments. UDP is
  supported as application-level datagram relay over the encrypted TCP mux tunnel.
- `local.tun.udp_timeout_secs` controls how long a single UDP relay waits for a
  response. Keep it short for desktop global TUN use so unanswered UDP probes do
  not occupy relay tasks for too long.
- Use `espejismo-local --doctor --tun-enabled --tun-auto-route --tun-auto-dns`
  before route takeover when possible. Doctor checks listener conflicts,
  server resolution/reachability, IPv4 server-route requirements, DNS inputs,
  and release-profile risks.

Crash recovery command:

```bash
sudo espejismo-local --config espejismo.toml --tun-route-cleanup
sudo espejismo-local --tun-name esptun0 --tun-route-cleanup
```

Systemd stop hook example:

```ini
[Service]
ExecStart=/usr/local/bin/espejismo-local --config /etc/espejismo.toml --tun-enabled --tun-auto-route --tun-auto-dns
ExecStopPost=/usr/local/bin/espejismo-local --config /etc/espejismo.toml --tun-route-cleanup
```

Linux manual route equivalent:

```bash
sudo ip rule add pref 80 to <remote-server-ip>/32 lookup main
sudo ip route replace default dev esptun0 table 20260
sudo ip rule add pref 81 lookup 20260
```

Restore Linux policy routing:

```bash
sudo ip rule del pref 81 lookup 20260
sudo ip rule del pref 80 to <remote-server-ip>/32 lookup main
sudo ip route flush table 20260
```

If another local TUN manager is running, such as Mihomo or Clash, check the
active routing and DNS ownership before testing:

```bash
ip rule show
ip route show table main
resolvectl status
```

Espejismo installs rules at priorities `80` and `81`, ahead of the common
high-numbered policy rules used by other local tunnel managers. Running multiple
global TUN/VPN managers at the same time is still not recommended because DNS
ownership and application routing expectations can conflict.

Windows manual route equivalent:

```powershell
route ADD <remote-server-ip> MASK 255.255.255.255 <current-gateway> METRIC 1 IF <physical-ifindex>
route ADD 0.0.0.0 MASK 128.0.0.0 10.255.0.1 METRIC 1 IF <tun-ifindex>
route ADD 128.0.0.0 MASK 128.0.0.0 10.255.0.1 METRIC 1 IF <tun-ifindex>
netsh interface ipv4 set dnsservers name="esptun0" static 1.1.1.1 primary
```

## Windows TUN Troubleshooting

Typical startup failure:

```text
Error: create TUN device esptun0
Caused by:
  0: LoadLibraryExW failed
  1: The specified module could not be found. (os error 126)
```

This means the Windows TUN runtime dependency is missing from the process DLL
search path. Espejismo on Windows requires the Wintun runtime (`wintun.dll`).
Official Windows release archives include `bin/wintun.dll` by default.

Recommended recovery steps:

1. Run the terminal as Administrator.
2. Ensure `wintun.dll` (matching process architecture) is placed next to
   `bin\espejismo-local.exe`, or in a standard DLL search location.
3. Use release package architecture that matches your OS
   (`windows-amd64` for most systems).
4. Re-run config diagnostics first:

```powershell
.\bin\espejismo-local.exe --config .\client.toml --tun-enabled --tun-auto-route --tun-auto-dns --check-config
```

5. Start TUN mode:

```powershell
.\bin\espejismo-local.exe --config .\client.toml --tun-enabled --tun-auto-route --tun-auto-dns
```

If route or DNS state becomes inconsistent after a crash:

```powershell
.\bin\espejismo-local.exe --config .\client.toml --tun-route-cleanup
```

macOS manual route equivalent:

```bash
sudo route -n add -host <remote-server-ip> <current-gateway>
sudo route -n add -net 0.0.0.0/1 -interface utun123
sudo route -n add -net 128.0.0.0/1 -interface utun123
sudo networksetup -setdnsservers Wi-Fi 1.1.1.1 8.8.8.8
```

Use `--check-config` first:

```bash
espejismo-local --config espejismo.toml --tun-enabled --tun-auto-route --check-config
espejismo-local --config espejismo.toml --tun-enabled --tun-auto-route --tun-auto-dns --doctor
espejismo-local --config espejismo.toml --probe-server
```

TCP-first smoke tests:

```bash
curl --max-time 20 http://1.1.1.1/cdn-cgi/trace
curl --max-time 20 https://ifconfig.me
```

The first command avoids DNS and checks whether TCP reaches the tunnel. The
second command also checks DNS takeover and UDP DNS relay.

With `--log-level debug`, a healthy Linux TUN startup should show the warm-up
before route installation:

```text
warming up tunnel before TUN route takeover
TUN warm-up stream opened
Linux TUN route manager installed
```
