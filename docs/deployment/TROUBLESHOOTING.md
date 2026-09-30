# Troubleshooting

Start with the local config diagnostics, service state, and recent logs. Avoid
sharing config dumps or logs that contain PSKs, proxy credentials, or admin
tokens.

```bash
espejismo-local --config /etc/espejismo/espejismo.toml --check-config
espejismo-local --config /etc/espejismo/espejismo.toml --doctor
espejismo-local --config /etc/espejismo/espejismo.toml --probe-server
sudo systemctl --no-pager --full status espejismo-remote
sudo journalctl -u espejismo-remote -n 100 --no-pager
```

Run `--check-config` with the corresponding binary on the remote host too.
`--probe-server` tests TCP reachability and completes the Espejismo handshake;
it does not start local proxy listeners.

| Symptom | Checks and likely action |
| --- | --- |
| Service exits immediately | Read `journalctl` for the first config, permission, bind, or startup error. Run that host's `--check-config`; verify the `User`/`Group` in the unit can read the config and any configured log file path. |
| Client cannot reach the server | Confirm `local.server` resolves to the intended public address and port; confirm `remote.listen` binds the expected interface; check host/cloud firewall allows that TCP port and that the process is listening. Run `--doctor`, then `--probe-server` from the client. |
| TCP connects but handshake fails | Confirm both sides use the same PSK (or matching user PSK), compatible shared settings, and synchronized clocks when handshake windows are enabled. Check the server logs without increasing verbosity globally. |
| Proxy listener unavailable | Check `local.socks5_listen` / `local.http_listen`, whether another process owns the port, and `--check-config` bind diagnostics. Point the application at the configured loopback address and port. |
| Proxy accepts a request but destination fails | Review remote egress allow/block host and port rules, DNS resolution, upstream proxy availability, and server-side logs. A restrictive policy can intentionally deny the destination. |
| Connections drop or reconnect repeatedly | Inspect `/status` or `/connections` when admin is enabled, plus both peers' logs. Check network stability, service restarts, resource limits, and whether both ends were upgraded/configured together. Admin runtime state is documented in [Admin and Metrics](ADMIN.md). |
| Noisy/large log files | Use journald or the platform log collector, or configure external rotation for `[logging].file`; Espejismo does not rotate file output. Use targeted filters such as `info,espejismo_core=debug` only while investigating. See [Logging](LOGGING.md). |
| Admin endpoint unavailable | Confirm `[admin].listen` is enabled and bound on the expected address, the service was restarted if the listener changed, and the request includes the configured bearer token. Keep it on loopback unless protected by a trusted firewall. |
| TUN traffic breaks after stop/crash | Restore route/DNS state with `espejismo-local --config <config> --tun-route-cleanup` (with the platform privileges required for route changes). Windows also supports cleanup without a config when the TUN name is supplied; see [CLI](CLI.md). |

For further diagnosis, collect binary versions, OS, sanitized config values,
the exact failing command, `--doctor` output, and a short relevant log window.
Do not include secrets. See [Configuration](CONFIG.md), [CLI](CLI.md), and
[Known Issues](../KNOWN_ISSUES.md) for related details.
