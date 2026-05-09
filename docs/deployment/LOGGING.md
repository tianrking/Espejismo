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

- `level`: tracing filter directive, for example `info`,
  `debug`, or `info,espejismo_core=debug`.
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
