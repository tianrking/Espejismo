# Frequently Asked Questions

## What does Espejismo do, and does it disguise traffic as HTTPS?

Espejismo carries client traffic through an authenticated encrypted tunnel to a
remote egress server. It does not impersonate TLS, HTTP, QUIC, or another
protocol, and it does not claim invisibility. Its protocol uses authenticated
encryption, masked metadata, and optional traffic shaping. See
[Project Positioning](../POSITIONING.md) and the [Protocol](../PROTOCOL.md).

## Which programs do I need, and where do they run?

Run `espejismo-remote` on the reachable server and `espejismo-local` on the
client machine. The client exposes local SOCKS5 and HTTP proxy listeners; TUN
is optional for system-level IPv4 traffic capture. Both sides can use the same
example TOML file with their respective sections configured. See the
[Quickstart](QUICKSTART.md).

## Which ports and firewall rules are required?

The server's configured `remote.listen` TCP port must be reachable from the
client; the example uses TCP `6690`. Allow that port in the server firewall and
cloud security group. The local SOCKS5 and HTTP listeners default to
`127.0.0.1:6680` and `127.0.0.1:6681`. Espejismo's tunnel uses TCP; relayed UDP
traffic is carried through that tunnel rather than a separate UDP underlay.
If the server is behind a router, forward the configured TCP listener port to
the server host. A client behind NAT usually needs no inbound port mapping.
See [NAT deployment](NAT.md), including CGNAT limitations.

## Why does the handshake fail?

Check that the client PSK matches the server's fallback PSK or the selected
user's PSK, that shared handshake settings match, and that both hosts have
reasonable clock synchronization when handshake windows are enabled. First run
`--check-config` on each host, then run `--probe-server` from the client. The
probe checks TCP reachability and completes a handshake; it does not start the
local proxy listeners. See [CLI diagnostics](CLI.md).

## Do matching release numbers guarantee that client and server can connect?

No. The authenticated handshake requires an exact wire protocol version, and
the peers do not negotiate a fallback. A configuration can also stop parsing
after an upgrade if a key was removed or renamed. Check the target release
notes, validate the saved config with each target binary's `--check-config`,
and coordinate upgrades unless that release explicitly documents compatibility.
See [version compatibility](VERSION-COMPATIBILITY.md) and
[upgrade and rollback](RUNBOOK.md).

## Does `--probe-server` start the proxy?

No. It checks the server connection and handshake only. Start `espejismo-local`
normally to expose SOCKS5 or HTTP listeners, then point applications at the
configured local address and port.

## Can I use UDP or QUIC applications?

SOCKS5 UDP relay and TUN UDP relay are supported over the TCP tunnel. TUN
blocks UDP destination port 443 by default, which lets browsers fall back to
TCP HTTPS; this can be changed with `local.tun.udp_block_ports` if needed. This
is not a physical UDP tunnel, and latency-sensitive UDP workloads may behave
differently from native UDP. See [TUN mode](TUN.md).

## Why can a destination still fail after the proxy connects?

The remote server performs destination DNS resolution and egress. Its configured
allow/block host and port rules, private-IP protections, DNS, or upstream proxy
can prevent a connection. Review the remote egress policy and server logs; see
[Egress](EGRESS.md) and [Troubleshooting](TROUBLESHOOTING.md).

## Does the installer configure a service or firewall?

No. The release installer downloads and extracts binaries and example configs.
You start the processes yourself or configure a service using the deployment
guides. Firewall and route changes are also managed by the operator; TUN route
takeover is optional and requires platform privileges. See [Systemd](SYSTEMD.md)
and [TUN mode](TUN.md).

## TUN routing stopped working after the client exited. How do I recover?

Route takeover changes host networking and may need elevated privileges. Run
`espejismo-local --config <config> --tun-route-cleanup` with the same config
and required platform privileges to restore saved route and DNS state. On
Windows, cleanup can also use the TUN interface name without a config. Check
`--doctor` and the TUN troubleshooting notes before enabling takeover again.
See [TUN mode](TUN.md) and [CLI diagnostics](CLI.md).

## Where should I look for logs, admin status, and more help?

Use `--doctor`, inspect the service logs, and consult the
[Troubleshooting guide](TROUBLESHOOTING.md). The optional admin endpoint exposes
runtime status when configured. Sanitize logs and config before sharing them:
never include PSKs, proxy credentials, or admin tokens.

## Is there a known transfer or performance limitation?

The known-issues page tracks an intermittent long-transfer failure on one
de-to-jp path and low default-profile throughput on high-RTT links. For the
latter, `--profile auto-throughput` is available. See
[Known Issues](../KNOWN_ISSUES.md) and [Benchmarks](../testing/BENCHMARKS.md).
