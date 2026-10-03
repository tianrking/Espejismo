# Backup documentation

## Findings and scope

The server and client use TOML configuration; credentials such as PSKs and
admin tokens live in that config. Code review found no database or durable
application data store. TUN routes are host operating-system state, and
`[logging].file` writes logs without internal rotation. The Docker example
bind-mounts its config, while the systemd unit reads `/etc/espejismo/`.

## Plan and expected result

- Add a deployment backup and recovery guide covering configs and secrets,
  deployment-specific files, TUN state, optional logs, and release artifacts.
- Give a restrictive-permission Linux copy example, validation commands, and
  restore checks. Link the guide from the deployment docs navigation.
- This is documentation only: no runtime behavior or performance changes are
  expected. The operational benefit is a clearer, safer recovery path and
  reduced risk of losing credentials or host-specific configuration.

## Validation

- Cross-checked configuration paths and log behavior against the deployment
  examples, systemd unit, Compose bind mount, and logging implementation.
- Reviewed links and commands against existing deployment guides and CLI.
- No application tests or throughput benchmark apply to this documentation-only
  change. No performance gain is claimed.
