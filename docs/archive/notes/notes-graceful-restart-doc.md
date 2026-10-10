# Graceful Restart Documentation

## Findings and approach

The service binaries treat Ctrl-C/SIGINT and (on Unix) SIGTERM as shutdown
signals. The local binary aborts and joins its listener tasks; the remote
stops its accept loop but does not join active peer handlers before returning.
Consequently, replacing either process does not transfer sessions or preserve
in-flight streams. Clients can reconnect after service returns, but an
interrupted application request or transfer must be retried. The sample
systemd restart sends SIGTERM, and its stop timeout does not implement a drain.

The existing `docs/deployment/SHUTDOWN.md` described prompt shutdown and
operator planning. This change adds the direct restart consequence and
role-specific connection effects there, cross-checked against the client and
server shutdown paths. No process behavior or protocol changes are proposed;
this keeps the small operations model and existing positioning intact.

## Expected effect

Operators can set accurate expectations for active SOCKS5, HTTP, and TUN
traffic while restarting either side, and distinguish application retry from
transparent stream resumption. Expected runtime/performance improvement: 0%,
because this is documentation-only. The benefit is clearer outage planning
and fewer incorrect assumptions about session continuity; it is not
quantified.

## Documentation check

- Cross-checked `crates/espejismo-client/src/main.rs` and
  `crates/espejismo-server/src/main.rs` shutdown paths and
  `docs/deployment/SYSTEMD.md`.
- `git diff --check` passes.
- Runtime tests and throughput benchmarks do not apply to this documentation
  change. No runtime correctness or performance behavior changed; no
  regression is expected.
