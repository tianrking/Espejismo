# PID Files And Multiple Instances

Espejismo does not create, read, or remove PID files, and neither binary has a
`--pidfile` option. Starting a binary directly leaves process supervision to
the invoking service manager or shell. Do not create a PID file by guessing a
PID after startup: the file can become stale, and Espejismo will not use it to
prevent a second process from starting.

## Use systemd process tracking

The supplied systemd units use `Type=simple`. systemd tracks the service
process and its cgroup, so a PID file is unnecessary. Use `systemctl status`,
`systemctl stop`, and `journalctl` to manage and inspect the process. See
[systemd deployment](SYSTEMD.md).

## Run multiple instances

Give each instance a separate config and a distinct systemd unit name. For
example, a remote service can use a template unit whose `ExecStart` names a
config for each instance:

```ini
[Unit]
Description=Espejismo remote instance %i
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=espejismo
Group=espejismo
ExecStart=/usr/local/bin/espejismo-remote --config /etc/espejismo/%i.toml
Restart=on-failure
RestartSec=3
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true
ProtectSystem=strict
ReadOnlyPaths=/etc/espejismo

[Install]
WantedBy=multi-user.target
```

Save this as `/etc/systemd/system/espejismo-remote@.service`, then create
separate readable config files such as `edge-a.toml` and `edge-b.toml`.
Validate and start each independently:

```bash
sudo -u espejismo /usr/local/bin/espejismo-remote --config /etc/espejismo/edge-a.toml --check-config
sudo -u espejismo /usr/local/bin/espejismo-remote --config /etc/espejismo/edge-b.toml --check-config
sudo systemctl daemon-reload
sudo systemctl enable --now espejismo-remote@edge-a espejismo-remote@edge-b
sudo systemctl status espejismo-remote@edge-a espejismo-remote@edge-b
```

Each instance must have non-conflicting bind addresses and ports for all
enabled listeners, including remote, local proxy, and admin listeners. Review
the effective configuration and firewall rules for each instance. Use separate
logs or journald unit names to make attribution clear. The same binary and
service account may be shared. For local TUN instances, also ensure each has
an intentionally planned interface and route ownership; multiple instances
must not compete for the same host routes or device.

For containers, use one container per instance with a distinct config, network
bindings, and container name. Do not rely on a PID file shared between
containers or instances.
