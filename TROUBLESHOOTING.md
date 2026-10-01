# Troubleshooting

Use this guide to locate which part of the tunnel is failing. Espejismo has a
local client, an authenticated TCP connection, and a remote egress process;
check those pieces in that order. The detailed deployment checklist is in
[`docs/deployment/TROUBLESHOOTING.md`](docs/deployment/TROUBLESHOOTING.md).

## 1. Validate both configurations

Run the matching binary on each host. This checks local settings before you
investigate the network:

```bash
espejismo-local --config ./espejismo.toml --check-config
espejismo-remote --config ./espejismo.toml --check-config
```

Confirm the client has the intended `local.server`, the server has the
intended `remote.listen`, and both sides have matching shared settings and
credentials. Keep the configuration file private; it can contain PSKs,
upstream proxy credentials, and admin tokens.

## 2. Check server reachability and handshake

On the server, confirm the process is running and listening on the configured
TCP address and port. Check host and cloud firewalls for that TCP port. From
the client, run:

```bash
espejismo-local --config ./espejismo.toml --doctor
espejismo-local --config ./espejismo.toml --probe-server
```

`--probe-server` checks TCP reachability and completes the Espejismo handshake;
it does not start local proxy listeners. If TCP is reachable but the handshake
fails, verify both peers use the same PSK (or the matching user PSK), compatible
shared settings, and synchronized system clocks when handshake windows are
enabled.

## 3. Check local proxy traffic

Confirm the client starts without bind errors and that the application points
to the configured `local.socks5_listen` or `local.http_listen` address. Check
whether another process already uses that port. Test SOCKS5 with:

```bash
curl --proxy socks5h://127.0.0.1:6680 https://example.com/
```

Replace the address and port with your configured listener. If the handshake
probe succeeds but this request fails, inspect the client log and continue to
the remote egress checks.

## 4. Check remote egress

If the proxy accepts a request but cannot reach its destination, inspect the
remote logs and `[remote.egress]` policy. Check DNS, allowed destination ports,
private-IP restrictions, and any configured upstream proxy. A policy denial
can be expected behavior; do not weaken it unless the destination should be
allowed.

## 5. Check service logs and reconnects

For systemd deployments, inspect service state and a short recent log window:

```bash
sudo systemctl --no-pager --full status espejismo-remote
sudo journalctl -u espejismo-remote -n 100 --no-pager
```

Check both peers for repeated restarts, network interruptions, and resource
limits. Increase logging only for the relevant Espejismo module while
investigating, then restore the normal level. Espejismo does not rotate its own
file logs; see [Logging](docs/deployment/LOGGING.md) for retention guidance.

## 6. Recover TUN routes after an abnormal stop

If TUN mode leaves routes or DNS settings behind, clean them up using the
original client config and the platform privileges needed to change routes:

```bash
sudo espejismo-local --config ./espejismo.toml --tun-route-cleanup
```

For Windows options and platform-specific route recovery, see the [CLI
reference](docs/deployment/CLI.md) and [TUN guide](docs/deployment/TUN.md).

## Known issue

Large transfers from de to jp have intermittently failed mid-transfer; the
cause is still under investigation. See [Known Issues](docs/KNOWN_ISSUES.md)
and [the stability notes](docs/notes-long-transfer-stability.md). High-RTT
links can also be slow with the default profile; see the
[benchmark guidance](docs/testing/BENCHMARKS.md) for the adaptive throughput
profile.

When asking for help, include binary versions, operating systems, the exact
failing command, and a short relevant log excerpt. Remove PSKs, proxy
credentials, admin tokens, and other private values first. For full symptom
coverage and deployment-specific checks, continue to
[`docs/deployment/TROUBLESHOOTING.md`](docs/deployment/TROUBLESHOOTING.md).
