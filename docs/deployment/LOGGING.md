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

## Audit and retention

Espejismo has no separate audit-log subsystem. Operational and security-related
events use the same `[logging]` filter, format, and output described above. For
an audit-oriented deployment, retain at least `info` events and send them to a
host-managed collector (journald by default, or the configured file path). JSON
is useful when the collector needs structured fields. `debug` and `trace` are
diagnostic levels, can produce much more data, and should be enabled only for a
focused investigation.

The logs include service lifecycle, accepted authenticated tunnels, and
authentication failures/timeouts. They are not a complete record of every
administrative request or configuration change, and local files are not
tamper-evident. Use the host's access controls, retention, and forwarding
policy when an external audit trail is required. Logs may contain destination
`target` values and configured `user` names; protect and retain them as
connection metadata. See [Admin](ADMIN.md) for admin endpoint access controls.

## Levels and formats

`error` indicates an operation could not continue, `warn` marks a rejected or
partially recovered operation, `info` records service lifecycle and successful
setup, `debug` adds per-connection and troubleshooting details, and `trace` is
for very detailed diagnostics. Normal operation should use `info`. Enable
`debug` or `trace` temporarily and narrow the filter to the relevant
application module when possible; verbose output can grow quickly.

With the default `compact` format, each event is a compact human-readable line
with its level, timestamp, structured fields, and message. `pretty` is a
multiline human-readable alternative. `json` emits one structured event per
line for log collectors; fields such as `peer`, `error`, `user`, and timing
values remain separate JSON properties. JSON disables ANSI regardless of the
`ansi` setting.

### JSON record fields

Each JSON line is one tracing event. The formatter supplies metadata fields
such as `timestamp`, `level`, `fields` (the event's message and recorded
fields), and `target` (the Rust module that emitted the event). Application
fields below are emitted only by the events that record them; they are not a
fixed schema present on every line. Timing values ending in `_ms` are integer
milliseconds, byte counts and counters are integers, and addresses, errors,
and enum-like values are formatted strings.

| Field | Meaning and where it appears |
| --- | --- |
| `role` | `local` or `remote` in `service started`. |
| `version` | Running binary version in `service started`. |
| `server` | Configured remote endpoint in the local `service started` event. |
| `listen` | Remote listener address in remote `service started`, or local proxy address in a proxy-listener-ready event. |
| `ingress` | Enabled local ingress summary (`socks5`, `http`, and/or TUN) in local `service started`. |
| `listeners` | Number of active remote listeners in remote `service started`. |
| `mux`, `underlay` | Configured mux and underlay modes in `service started`. |
| `peer` | Remote socket address associated with a connection event, where recorded. |
| `user` | Configured authenticated user on `authenticated tunnel accepted` and selected per-flow diagnostics. This is an account name, not a credential. |
| `error` | Human-readable cause on failures and cleanup warnings. The value can vary by operating system and failure. |
| `target` | Requested destination hostname or authority in flow events. Treat it as connection metadata. |
| `priority` | Logical stream priority on applicable flow events. |
| `lane_id` | Local tunnel lane selected for a flow. |
| `handshake_ms`, `cold_start_ms` | Authentication handshake duration and configured post-authentication cold-start delay elapsed before the tunnel is accepted. |
| `accept_ms`, `open_ms`, `request_ms`, `prebuffer_ms`, `egress_connect_ms`, `copy_ms`, `total_ms` | Durations for the corresponding accept, tunnel-open, request, prebuffer, remote egress-connect, data-copy, or complete-flow phase. Only fields relevant to that flow type are present. |
| `client_to_remote`, `remote_to_client` | Bytes copied in each direction for a completed stream. |
| `remote_to_client_bps` | Approximate remote-to-client throughput for the measured copy phase. |

The formatter also records the event message in `fields.message`. For example,
`authenticated tunnel accepted` is the message, while `user`, `handshake_ms`,
and `cold_start_ms` are independent fields. Exact optional fields depend on
the event and the path taken; consumers should tolerate missing fields and
unknown future fields.

## Reading common events

| Event or field | Meaning and next step |
| --- | --- |
| `service started` | The process started; `role`, `mux`, and `underlay` describe active setup. Remote records also include `listen` and `listeners`; local records include `server` and `ingress`. |
| `SOCKS5 proxy listening` / `HTTP proxy listening` | The local proxy listener is ready at the `listen` address. Check the configured address if a local application cannot connect. |
| `authenticated tunnel accepted` | The remote accepted a peer; `user`, `handshake_ms`, and `cold_start_ms` show identity and setup timings. |
| `peer authentication failed` / `peer authentication timed out` | A remote handshake was rejected or exceeded its configured deadline. Check that client and server credentials and protocol settings match, then inspect the associated `error`; unauthenticated peers may also be expected internet traffic. |
| `remote peer dropped because global connection limit is full` | The configured server connection cap is reached. Check current load and connection limits. |
| `perf local stream completed` | A local proxy flow completed. `open_ms`, `copy_ms`, byte counts, and `remote_to_client_bps` help distinguish setup delay from transfer time; this is emitted at `debug`. |
| `TUN route restore was incomplete` | Route cleanup encountered an error during shutdown or recovery. Read `error` and verify the host routes and DNS settings before relying on the system network. |
| `shutdown signal received` | The process received its normal shutdown signal. |

Application events use structured fields where available. In particular,
`target` fields can contain requested destination hostnames, and `user` fields
identify configured users. Treat collected logs as operational data and apply
the same access and retention controls used for other connection metadata.

For a focused investigation, start with a module filter such as
`info,espejismo_server=debug` or `info,espejismo_client=debug`. A global
`debug` or `trace` enables that level for Espejismo application crates while
keeping high-volume transport dependencies capped at `info`; see the
dependency note below.

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

### Syslog and centralized collectors

The binaries write formatted events to stderr (or to the configured file); they
do not open a syslog socket or emit RFC 3164/5424 messages directly. Under the
supplied systemd units, journald is the integration point. This keeps the
service configuration small and lets the host forward records using its
existing rsyslog or syslog-ng setup.

To give journal records a predictable program name, add a systemd drop-in:

```ini
# /etc/systemd/system/espejismo-remote.service.d/logging.conf
[Service]
SyslogIdentifier=espejismo-remote
StandardOutput=journal
StandardError=journal
```

Apply the drop-in and confirm the identifier:

```bash
sudo systemctl daemon-reload
sudo systemctl restart espejismo-remote
sudo journalctl -t espejismo-remote -f
```

For rsyslog forwarding, enable its journal input (`imjournal`) according to the
host's rsyslog packaging, then add a rule such as:

```conf
# /etc/rsyslog.d/40-espejismo.conf
if $programname == 'espejismo-remote' then {
    action(type="omfwd" target="logs.example.net" port="6514"
           protocol="tcp" StreamDriver="gtls" StreamDriverMode="1"
           StreamDriverAuthMode="x509/name"
           StreamDriverPermittedPeers="logs.example.net")
    stop
}
```

Replace the collector name and certificate settings with the site's trusted
TLS configuration. Configure the matching client certificate and trust policy
on both ends before forwarding production logs; plain TCP forwarding exposes
log contents in transit. Keep local journal retention as a buffer and verify
delivery at the collector after restarting rsyslog. If the service is run
outside systemd, send stderr through the host's syslog supervisor or collector;
setting `[logging].file` alone does not send records to syslog.

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

## OpenTelemetry and export paths

The application uses the Rust `tracing` API to create structured diagnostic
events, but the shipped binaries do not install an OpenTelemetry subscriber or
an OTLP exporter. Setting standard `OTEL_*` environment variables therefore
does not send traces or logs to an OpenTelemetry collector. The `tracing`
events documented above are formatted locally to stderr (journald under the
provided systemd units) or to the configured `[logging].file`; forward those
logs with the host's existing collector integration when centralized logs are
needed.

Metrics use a separate, pull-based path: scrape the authenticated admin
`GET /metrics` endpoint with Prometheus. It emits Prometheus text exposition,
not OTLP metrics. Keep the admin listener on loopback or behind a trusted
firewall, and configure the scrape credential as shown in
[Monitoring and Alerting](MONITORING-ALERTS.md). See
[Prometheus Metrics](METRICS.md) for the exported series and their scope.

There is no distributed trace context propagation or span export in the
current binaries. The structured `peer`, `user`, `target`, and timing fields
in log events are useful for local diagnosis, but should not be treated as
trace/span identifiers or a complete request trace.
