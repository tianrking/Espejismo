# Logging

Espejismo uses `tracing` across local, remote, and core protocol modules. Both
`espejismo-local` and `espejismo-remote` read the same `[logging]` config and
accept the same command-line overrides.

## Configuration

```toml
[logging]
level = "info"
format = "compact"
ansi = true
# file = "/var/log/espejismo/espejismo.log"
```

Fields:

- `level`: tracing filter directive, for example `info`, `debug`, or
  `info,espejismo_core=debug`. A global `debug` or `trace` level is interpreted
  as application-only verbosity for `espejismo_core`, `espejismo_client`, and
  `espejismo_server`; high-volume transport dependencies stay capped at `info`.
- `format`: `compact`, `pretty`, or `json`.
- `ansi`: whether human-readable console output should use ANSI color.
- `file`: optional log file path. Parent directories are created if missing.

JSON logs always disable ANSI escape sequences.

## Command-Line Overrides

```bash
./bin/espejismo-remote \
  --config configs/espejismo.toml \
  --log-level 'info,espejismo_core=debug' \
  --log-format json \
  --log-file /var/log/espejismo/remote.log \
  --no-log-ansi
```

The same flags work for `espejismo-local`.

## Operational Notes

File output is intentionally simple and cross-platform. It writes to the exact
configured file and does not rotate logs internally. Use systemd journald,
Docker logging drivers, `logrotate`, or the platform's native log collector for
retention and rotation.

### Production retention on Linux

With the supplied systemd units, the default is stderr, which systemd sends to
journald. Prefer that path unless a separate log file is required:

```bash
sudo journalctl -u espejismo-remote --since today
sudo journalctl --disk-usage
```

For example, create `/etc/systemd/journald.conf.d/espejismo-retention.conf`:

```ini
[Journal]
SystemMaxUse=1G
SystemKeepFree=2G
MaxRetentionSec=14day
```

Choose values for the host's disk and all services: these are host-wide limits
shared with other journal entries, not per-service quotas. Apply changes with
`sudo systemctl restart systemd-journald`; inspect the effective policy with
`systemd-analyze cat-config systemd/journald.conf`.

If `[logging].file` is configured, install a logrotate rule on each host. The
example assumes `/var/log/espejismo/espejismo.log`; change the path to match
`logging.file`. The supplied units run as `espejismo`, so the log directory
must be writable by that account.

```conf
# /etc/logrotate.d/espejismo
/var/log/espejismo/espejismo.log {
    daily
    size 50M
    rotate 14
    maxage 14
    compress
    delaycompress
    missingok
    notifempty
    copytruncate
}
```

`copytruncate` keeps the running process writing to the configured path and
avoids a service restart. A small amount of data can be lost between copying
and truncating during rotation. `size` is checked when logrotate runs, so it
does not impose an instantaneous hard file-size cap. If that tradeoff is unacceptable, remove
`copytruncate` and add a `postrotate` action that restarts the matching
service so it reopens the file; restarting interrupts active tunnel sessions. The
application does not reopen the file on its own, so rename-and-create without
either approach leaves it writing to the rotated file. Check the rule with
`sudo logrotate --debug /etc/logrotate.d/espejismo`, then force one test
rotation with `sudo logrotate --force /etc/logrotate.d/espejismo` and inspect
ownership, file sizes, and service status. The rule bounds routine retention;
allow extra disk for a rotation in progress and compressed archives.

The logger intentionally suppresses dependency frame-body dumps from crates such
as `tokio_yamux`. Those logs can contain huge per-frame payload renderings and
can dominate disk I/O during throughput tests. Use Espejismo module filters,
for example `info,espejismo_core=debug`, when debugging application behavior.

## Metrics

The admin endpoint `GET /metrics` exposes Prometheus text format. Use the
cumulative byte counters to graph traffic rates (Prometheus computes bytes/s):

```promql
rate(espejismo_bytes_client_to_remote_total[5m])
rate(espejismo_bytes_remote_to_client_total[5m])
```

The current endpoint does not report transport retransmissions or live mux
window occupancy. UDP reliability helpers are not wired into a production
path, and mux windows are configured per stream; do not interpret the byte
counters or configured window as retransmission/loss telemetry.
