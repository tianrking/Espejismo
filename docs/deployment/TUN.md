# Native TUN Mode

TUN mode is an optional local ingress for system-level traffic capture. It is
disabled by default and does not change the stable SOCKS5/HTTP proxy path.

```toml
[local.tun]
enabled = true
name = "esptun0"
address = "10.255.0.2"
prefix = 24
destination = "10.255.0.1"
mtu = 1500

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
  --tun-dns 1.1.1.1,8.8.8.8
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
- `espejismo-remote` is unchanged; existing users, quota, bandwidth, egress,
  logging, and admin status still apply.

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
- Windows route takeover is also opt-in. It protects the remote server route
  through the current default gateway and installs split-default
  `0.0.0.0/1` plus `128.0.0.0/1` routes through the TUN interface, leaving the
  original default route intact for recovery.
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
```
