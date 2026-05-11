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

How it works:

- `espejismo-local` creates a native TUN interface.
- A userspace netstack converts TUN TCP flows into existing tunnel TCP CONNECT
  streams.
- UDP datagrams are converted into existing tunnel UDP relay requests.
- `espejismo-remote` is unchanged; existing users, quota, bandwidth, egress,
  logging, and admin status still apply.

Important deployment notes:

- Creating a TUN interface usually requires root/admin privileges or a platform
  VPN entitlement.
- Linux route takeover is opt-in through `[local.tun.route].enabled = true` or
  `--tun-auto-route`.
- When route takeover is enabled, Espejismo first resolves `local.server`, adds
  direct `/32` routes for the remote IPv4 addresses through the original default
  gateway, then replaces the default route with the TUN device.
- DNS takeover is opt-in through `[local.tun.route].dns_enabled = true` or
  `--tun-auto-dns`. On Linux it uses `resolvectl dns`, `resolvectl domain ~.`,
  and `resolvectl default-route yes`.
- On Ctrl-C or SIGTERM, Espejismo restores the original default route, reverts
  the TUN DNS settings, and removes the protected remote routes on a best-effort
  basis.
- The current TUN implementation is intended for TCP-first deployments. UDP is
  supported as application-level datagram relay over the TCP/yamux tunnel.

Linux manual route equivalent:

```bash
sudo ip route add <remote-server-ip>/32 via <current-gateway>
sudo ip route replace default dev esptun0
```

Restore Linux default routing with your original gateway:

```bash
sudo ip route replace default via <current-gateway> dev <physical-interface>
```

Use `--check-config` first:

```bash
espejismo-local --config espejismo.toml --tun-enabled --tun-auto-route --check-config
```
