# Logging documentation notes

## Findings and scope

`docs/deployment/LOGGING.md` already described configuration, output formats,
file retention, and metrics, but did not explain what the levels mean or how
to interpret common events. I checked the logging initializer and the client,
server, and TUN call sites before documenting examples. The initializer maps a
global `debug` or `trace` directive to Espejismo application crates and caps
noisy transport dependencies at `info`. Important events include server
startup, authenticated tunnel acceptance, authentication rejection/timeouts,
connection-cap drops, per-flow performance summaries, and TUN route restoration
warnings.

The documentation now explains level and format choices, shows the practical
meaning of these events and fields, and calls out that destination `target`
and configured `user` fields are connection metadata. This remains consistent
with the project's small-operations model and does not change logging behavior
or the protocol.

## Expected result

Operators can use normal `info` output for lifecycle and setup, temporarily
enable targeted application `debug` output to diagnose connections, and read
common startup/authentication/flow/TUN events without searching source code.
No runtime or throughput change is expected from this documentation-only
update.

## Review / experiment

- Cross-checked documented levels and format behavior against
  `crates/espejismo-core/src/logging.rs` and `LogConfig`.
- Cross-checked event names and fields against the client/server handlers and
  TUN route managers.
- This change only edits Markdown; no executable behavior changed, so cargo
  tests and throughput benchmarks are not applicable.
