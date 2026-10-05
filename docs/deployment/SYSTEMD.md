# systemd deployment

The repository includes example units for the ordinary proxy services:
[`espejismo-remote.service`](../../deployments/systemd/espejismo-remote.service)
and [`espejismo-local.service`](../../deployments/systemd/espejismo-local.service).
They run the matching binary as the unprivileged `espejismo` account, read the
TOML file from `/etc/espejismo/espejismo.toml`, restart after failures, and send
stderr logs to journald.

## Install and enable

Create a dedicated service account and protected config directory, install the
matching binary and unit, then validate the config before starting:

```bash
sudo useradd --system --no-create-home --shell /usr/sbin/nologin espejismo
sudo install -d -o root -g espejismo -m 0750 /etc/espejismo
sudo install -o root -g espejismo -m 0640 espejismo.toml /etc/espejismo/espejismo.toml
sudo install -o root -g root -m 0644 deployments/systemd/espejismo-remote.service /etc/systemd/system/
sudo install -o root -g root -m 0755 espejismo-remote /usr/local/bin/
sudo -u espejismo /usr/local/bin/espejismo-remote --config /etc/espejismo/espejismo.toml --check-config
sudo systemctl daemon-reload
sudo systemctl enable --now espejismo-remote
sudo systemctl status espejismo-remote
```

For a client service, install `espejismo-local.service` and
`espejismo-local` instead. Run only the role needed on the host. Keep the
configuration root-owned and readable by the service group because it contains
PSKs and may contain admin tokens. Apply updates by replacing the binary/config
and restarting the unit. See [Configuration](CONFIG.md) and
[Version compatibility](VERSION-COMPATIBILITY.md).

Inspect startup and runtime logs with:

```bash
sudo journalctl -u espejismo-remote -f
```

The sample units already use `NoNewPrivileges`, `PrivateTmp`, `ProtectHome`,
`ProtectSystem=strict`, and a read-only config path. These restrictions suit
ordinary SOCKS5/HTTP proxy mode, which does not need elevated privileges or
writable host paths. Keep file logging disabled for this layout; use journald
or explicitly grant a service-owned log directory. For retention settings see
[Logging](LOGGING.md).

Espejismo does not manage PID files; systemd tracks the service process
directly. To run multiple instances, use separate unit instances and config
files with non-conflicting listeners. See [PID files and multiple instances](PIDFILES.md).

## Optional additional hardening

On hosts where the installed systemd version supports these directives, an
operator can add the following under `[Service]` in a drop-in or copied unit:

```ini
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
LockPersonality=true
RestrictSUIDSGID=true
```

Apply a drop-in with `sudo systemctl edit espejismo-remote`, then run
`sudo systemctl daemon-reload` and restart the service. Check the host's systemd
version and review `sudo systemd-analyze security espejismo-remote.service`;
that report is a review aid, not a guarantee that every restriction is
compatible with every host or deployment.

TUN mode is different from ordinary proxy mode: it needs access to the TUN
device and network administration privileges to configure routes. Do not use
`PrivateDevices=true` or drop the required capability for a TUN client. Grant
only the permissions needed for the host's TUN setup, and follow the
[TUN service guidance](TUN.md#systemd-stop-hook-example) for route cleanup.
The supplied local unit intentionally does not grant these privileges by
default.

## Operations

```bash
sudo systemctl restart espejismo-remote
sudo systemctl stop espejismo-remote
sudo systemd-analyze verify /etc/systemd/system/espejismo-remote.service
```

Stopping sends SIGTERM. Active streams can be interrupted during a restart;
systemd stop timeouts do not provide connection draining. See
[Shutdown behavior](SHUTDOWN.md).
