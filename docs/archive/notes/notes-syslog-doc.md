# Syslog integration documentation notes

## Findings and scope

`espejismo-core` initializes `tracing_subscriber` with stderr when
`[logging].file` is absent, and with a file appender when it is set. It has no
syslog socket or RFC 3164/5424 output layer. The supplied systemd units run the
binary normally, so stderr is collected by journald. The existing logging and
systemd guides described journald but did not connect that behavior to
traditional syslog forwarding.

Documented the actual integration path in `docs/deployment/LOGGING.md`: systemd
drop-in identifier and journal settings, journal inspection, and an rsyslog
forwarding example that assumes the host's `imjournal` input. The text calls
out TLS trust configuration and cautions that a configured file is not itself
a syslog destination. No application behavior, configuration schema, or
deployment model changed.

## Expected result

Operators using systemd can route service records into existing syslog-based
collectors through journald without adding a syslog dependency to the service.
No runtime, resource, or throughput change is expected from this documentation
update.

## Review / experiment

- Checked output behavior against `crates/espejismo-core/src/logging.rs` and
  the supplied remote systemd unit.
- Reviewed the Markdown diff and command/config examples for consistency with
  `docs/deployment/SYSTEMD.md`.
- Documentation only: compilation, tests, and throughput benchmarking are not
  applicable; no measured performance change is claimed.
