# Log rotation documentation

## Findings and scope

`espejismo-core` initializes a `tracing_appender::rolling::never` writer for
`[logging].file`. It opens one exact path and has no reopen signal or internal
rotation. The supplied systemd units leave output on stderr, which journald
collects. The existing logging guide mentioned external collectors but gave no
retention configuration or explanation of the file-handle behavior.

## Plan and expected result

- Document journald as the default Linux path, with an example host-wide
  retention drop-in and commands to inspect effective limits.
- Provide a logrotate example for configured file output with daily checks,
  size threshold, bounded archive count/age, compression, and `copytruncate`.
- Explain the small copy/truncate loss window, that size is checked on the
  logrotate schedule rather than being a hard cap, and the restart needed by a
  rename-and-create alternative.

This is documentation only. It adds no runtime work, dependencies, or claimed
performance improvement. The expected operational benefit is that an operator
can configure bounded log retention using the system's existing collector and
avoid indefinite file growth.

## Validation

- Reviewed the logging initializer and supplied systemd units against the
  documented behavior: file logging uses `rolling::never`; units use the
  default stderr stream.
- Reviewed the examples and paths for consistency with the `[logging].file`
  setting and the supplied service account.
- No application tests or throughput benchmark apply to this documentation-only
  change. No performance gain is claimed.
