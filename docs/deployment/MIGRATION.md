# Migration from Other Proxy Tools

This guide moves a client and server from another proxy tool to Espejismo. It
does not convert another tool's configuration or make its protocol compatible
with Espejismo. Plan to install `espejismo-remote` on the egress server and
`espejismo-local` on each client, then copy the settings you need into TOML.

Espejismo deliberately does not impersonate TLS, QUIC, or another protocol.
Its authenticated encrypted tunnel has its own handshake and wire format. A
TLS certificate, VLESS UUID, Shadowsocks method/password, or Hysteria password
cannot be reused as an Espejismo credential. Create a new, unique, long random
PSK and protect it like a password.

## What maps across

| Existing setup concept | Espejismo setting | Migration note |
| --- | --- | --- |
| Server address and port | `[local].server` | Point to the new Espejismo server listener. |
| Local SOCKS5 listener | `[local].socks5_listen` | Applications can keep using a local SOCKS5 proxy. |
| Local HTTP proxy listener | `[local].http_listen` | This is an HTTP proxy listener, not an HTTP tunnel transport. |
| Server bind address | `[remote].listen` | Permit the selected TCP port through the server firewall. |
| Shared secret | `[shared].psk` and server user PSK | Generate a new PSK; configure the same value on the matching client and server user. |
| Remote destination restrictions | `[remote.egress]` | Recreate host and port allow/block rules deliberately; do not assume old routing rules transfer. |
| Upstream egress proxy | `[remote.egress].proxy` | Optional server-side proxy; supported schemes and UDP limitations are in [Egress Policy](EGRESS.md). |
| System-wide client routing | `[local.tun]` and `[local.tun.route]` | Optional TUN mode; follow the platform-specific [TUN guide](TUN.md) and verify server-route protection. |

Espejismo provides TCP, WebSocket, and cleartext prior-knowledge HTTP/2
underlays. These are transport options for Espejismo's own encrypted tunnel;
they do not recreate another tool's TLS/ALPN camouflage or protocol behavior.
See [`shared.underlay`](CONFIG.md#sharedunderlay) before selecting a non-default
underlay. The simplest migration starts with TCP.

## Step-by-step cutover

1. **Inventory the current deployment.** Record the server hostname and
   reachable port, local proxy listener addresses, which applications use the
   proxy, destination restrictions, and whether traffic depends on UDP,
   per-domain routing, multiple outbound chains, or protocol-specific features.
2. **Check feature fit.** Espejismo is a focused encrypted tunnel, not a
   multi-protocol routing core. Compare required behavior with [Configuration](CONFIG.md),
   [Egress Policy](EGRESS.md), and [TUN Mode](TUN.md). Keep the current service
   available until the replacement passes your checks.
3. **Install both Espejismo binaries.** Install the release package on the
   remote egress host and client machine using the
   [deployment quickstart](QUICKSTART.md). Espejismo needs its own remote
   endpoint; a still-running Xray, sing-box, Hysteria 2, or Shadowsocks server
   cannot accept the Espejismo tunnel.
4. **Create a minimal config.** Start from the example below. Use a new random
   PSK, set the public server address on the client, and allow the chosen TCP
   port through host and cloud firewalls. For a single user, the server can
   authenticate with `shared.psk`; for multiple users, configure
   `[[remote.users]]` on the server and give each client only its own PSK.
5. **Validate before switching applications.** Run `--check-config` on both
   sides, start the remote, then run the local `--probe-server`. Test one
   application through SOCKS5 or HTTP and check the server logs and egress
   policy. UDP-dependent applications need separate validation: SOCKS5 UDP
   relay is supported, but the tunnel transport itself uses TCP.
6. **Move clients gradually.** Point one application at the new local proxy
   listener. Once it works, move the remaining applications or enable TUN
   routing only after following the TUN guide. Keep a note of the previous
   listener and route settings so you can restore them during the cutover
   window.
7. **Retire the old endpoint after observation.** Confirm expected destinations,
   DNS behavior, and resource use first. Remove old credentials only when no
   clients depend on the previous service.

Minimal single-user starting point (use the same PSK on both hosts):

```toml
[shared]
psk = "replace-with-a-new-long-random-secret"

[local]
server = "YOUR_SERVER_HOST:6690"
socks5_listen = "127.0.0.1:6680"
http_listen = "127.0.0.1:6681"

[remote]
listen = "0.0.0.0:6690"

[remote.egress]
deny_private_ips = true
allow_ports = [80, 443]

[logging]
level = "info"
format = "compact"
```

On the server:

```bash
espejismo-remote --config espejismo.toml --check-config
espejismo-remote --config espejismo.toml
```

On the client:

```bash
espejismo-local --config espejismo.toml --check-config
espejismo-local --config espejismo.toml --probe-server
espejismo-local --config espejismo.toml
```

Configure an application to use `127.0.0.1:6680` (SOCKS5) or
`127.0.0.1:6681` (HTTP). The full config reference covers users, limits,
underlays, and all supported settings.

## Import and export boundaries

`--import-profile` accepts only Espejismo's own `espejismo://import/...` client
profile format. It is useful for sharing Espejismo client settings after the
server has been configured; it does not read sing-box JSON, Xray JSON, SIP002
Shadowsocks links, or Hysteria 2 URIs. Likewise, `--print-client-profile`
creates an Espejismo profile and not a universal subscription link. Profile
URLs contain the PSK and must be handled as secret material. See
[Client Profiles](PROFILES.md#client-import-profiles) for details.
