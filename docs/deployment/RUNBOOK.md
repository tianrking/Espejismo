# Deployment, Upgrade, And Rollback Runbook

This runbook covers a small Linux deployment using the supplied systemd units.
The release installer only extracts binaries and example files; it does not
install services, open firewall ports, or change routes. Adapt paths and service
names if you manage processes another way. Keep the client and remote on
compatible configuration and protocol versions.

## Initial deployment

1. Install the release package and copy the example config to a protected
   location. The server-only package is sufficient on the remote host.
2. Set a long random `[shared].psk` (or configure matching `remote.users`),
   `remote.listen`, and the client's `local.server`. Review
   `[remote.egress]`; enable `deny_private_ips` unless private destinations are
   intentionally required. Keep admin bound to loopback and set a strong token
   if it is exposed beyond loopback.
3. Validate the config on each host before starting:

   ```bash
   /usr/local/bin/espejismo-remote --config /etc/espejismo/espejismo.toml --check-config
   /usr/local/bin/espejismo-local --config /etc/espejismo/espejismo.toml --check-config
   ```

4. Install the matching unit from `deployments/systemd/`, create the
   `espejismo` user/group and config directory, and grant read access to the
   config. The units run as that unprivileged account. Enable and start the
   remote first, then verify its listener/firewall and probe it from the client:

   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable --now espejismo-remote
   sudo systemctl status espejismo-remote
   # On the client, after its config is in place:
   espejismo-local --config /etc/espejismo/espejismo.toml --probe-server
   ```

   Install/start `espejismo-local` similarly where a persistent local proxy is
   wanted. The supplied local unit is for ordinary proxy mode; TUN route/DNS
   takeover needs elevated network privileges and explicit cleanup planning.

5. Verify the configured SOCKS5/HTTP listener with an application, and inspect
   service logs. If admin is enabled, check `/healthz` and `/status` locally.

## Upgrade

1. Select and pin the target release. Back up the current binaries and config;
   record the current version (`espejismo-remote --version`) and preserve the
   exact config, including PSKs and admin credentials, with restrictive
   permissions. The installer extracts in place and does not manage services.
2. Check release notes for compatibility/config changes. Stage the replacement
   binaries outside the live path when practical, then run both new binaries'
   `--check-config` against the saved production config.
3. Stop the service, replace the relevant binary (remote package on server,
   full package or local binary on client), and start it again:

   ```bash
   sudo systemctl stop espejismo-remote
   sudo install -m 0755 ./espejismo-remote /usr/local/bin/espejismo-remote
   sudo systemctl start espejismo-remote
   sudo systemctl --no-pager --full status espejismo-remote
   ```

   Repeat for `espejismo-local` where used. A restart causes active tunnel
   sessions to reconnect. Runtime admin reload/apply can update supported
   settings without a restart; process-owned listeners, TUN ownership, and log
   file handles still require restart (see [Admin](ADMIN.md)).
4. Confirm the reported version, a successful client `--probe-server`, proxy
   traffic, and clean logs before removing the backup.

## Rollback

1. Stop the affected service and restore its saved binary and matching config
   backup. Keep client/server configs paired if a change altered PSKs or shared
   protocol settings.
2. Start the service, check its status and logs, then run `--check-config`,
   client `--probe-server`, and a small application request.
3. If rollback follows a config-only change, restore the config first. Do not
   rotate the PSK during rollback unless both peers are updated together.

## Routine checks

```bash
sudo systemctl --no-pager --full status espejismo-remote
sudo journalctl -u espejismo-remote -n 100 --no-pager
espejismo-local --config /etc/espejismo/espejismo.toml --doctor
espejismo-local --config /etc/espejismo/espejismo.toml --probe-server
```

For metrics and runtime connection details see [Admin and Metrics](ADMIN.md);
for log levels and retention see [Logging](LOGGING.md). Espejismo does not
rotate its own file logs. TUN users should keep the config available for
`--tun-route-cleanup` after abnormal termination; see [CLI](CLI.md) and
[TUN mode](TUN.md).
