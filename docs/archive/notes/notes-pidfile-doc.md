# PID File And Multiple Instance Documentation

## Findings and approach

Reviewed both binary CLI definitions, the deployment CLI reference, and the
sample systemd units. Neither binary implements PID file creation, locking,
cleanup, or a `--pidfile` option. The shipped units use `Type=simple`, which
lets systemd track the process without an application-managed PID file.

Added a deployment guide that states this behavior and demonstrates systemd
template instances with a separate config per instance. It calls out distinct
listener ports, per-unit log attribution, and the extra route/device planning
needed for TUN. Linked it from the CLI and systemd guides. This documentation
keeps process supervision with the service manager and does not add daemon or
PID-file behavior to Espejismo.

Expected runtime performance improvement: 0% (documentation only). The
operational benefit is avoiding stale or misleading PID files and showing how
to operate multiple independently configured services; that benefit is not
quantified.

## Verification

Cross-checked the statements against `crates/espejismo-client/src/main.rs`,
`crates/espejismo-server/src/main.rs`, and
`deployments/systemd/espejismo-{local,remote}.service`. Searched CLI and source
for PID-file options and implementation; none were found. Documentation-only
change: runtime tests and throughput benchmarks do not apply. No runtime
regression is introduced.
