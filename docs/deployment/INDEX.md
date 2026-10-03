# Operations Documentation Index

Use this page to find the operational guide for a task. Espejismo keeps its
runtime model to one local binary, one remote binary, and one TOML file; the
guides below describe the existing behavior and operator-managed deployment
choices.

## Start here

- [Deployment quickstart](QUICKSTART.md) — install a release and bring up a
  basic client and server.
- [Frequently asked questions](FAQ.md) — common setup and behavior questions.
- [Deployment, upgrade, and rollback runbook](RUNBOOK.md) — staged Linux
  service deployment, upgrade, rollback, and routine checks.
- [Troubleshooting](TROUBLESHOOTING.md) — diagnose configuration, connectivity,
  service, and traffic problems.
- [CLI reference](CLI.md) and [process exit codes](EXIT-CODES.md) — command
  options and process outcomes.

## Install and operate

- [systemd deployment](SYSTEMD.md) — supplied Linux service units.
- [Docker deployment](DOCKER.md) — container image and Compose example.
- [Windows deployment](WINDOWS.md) — Windows install, firewall, and TUN notes.
- [Packaging](PACKAGING.md) — release archives and package contents.
- [Update checks](UPDATES.md) — check release metadata without replacing a
  running binary.
- [Client/server version compatibility](VERSION-COMPATIBILITY.md) — protocol
  compatibility and upgrade pairing.
- [Shutdown and connection draining](SHUTDOWN.md) — signal handling and
  current shutdown behavior.
- [PID files and multiple instances](PIDFILES.md) — process tracking and
  service-manager guidance.
- [Environment variables](ENVIRONMENT.md) — runtime and installer variables.

## Configure security and networking

- [Security documentation index](../SECURITY.md) — security behavior,
  credential handling, access controls, and reporting.
- [Configuration reference](CONFIG.md) — TOML settings and related guides.
- [Configuration examples](CONFIG-EXAMPLES.md) — maintained full config and
  complete deployment scenarios.
- [Authentication and key management](AUTHENTICATION.md) — PSKs, users, and
  credential rotation.
- [Users, quotas, and bandwidth limits](USERS.md) — configure independent
  user credentials and per-user limits.
- [Egress policy](EGRESS.md) — outbound destination controls.
- [Traffic shaping and obfuscation](OBFUSCATION.md) — profiles, padding, and
  observable traffic properties.
- [Profiles](PROFILES.md) — built-in configuration overlays.
- [NAT deployment](NAT.md) and [DNS behavior](DNS.md) — reachability and name
  resolution paths.
- [IPv6 deployment notes](IPV6.md) — IPv6 support and TUN limitations.
- [TLS certificates](TLS-CERTIFICATES.md) — explains the tunnel's native
  authentication and certificate boundaries.
- [SOCKS5 ingress](SOCKS5.md) and [HTTP proxy ingress](HTTP.md) — application
  proxy listeners.
- [Native TUN mode](TUN.md) — system traffic capture, routes, and DNS takeover.
- [Stream flow control](STREAM-FLOW-CONTROL.md) and [traffic priority/QoS](QOS.md)
  — stream-level scheduling behavior and limits.

## Recover and respond

- [Backup and recovery](BACKUP.md) — protect configuration and restore a host.
- [On-call response](ONCALL.md) — triage an incident and verify recovery.
- [Incident postmortem template](POSTMORTEM-TEMPLATE.md) — record a significant
  incident without including secrets.
- [High availability](HIGH-AVAILABILITY.md) — operator-managed endpoint
  failover and its limits.
- [Migration from other proxy tools](MIGRATION.md) — move settings and
  credentials to Espejismo.
- [Error and status reference](ERRORS.md) — interpret diagnostics and protocol
  status values.

## Monitor and tune

- [Admin and metrics](ADMIN.md) — optional authenticated admin endpoints.
- [Health checks](HEALTHCHECK.md) — liveness endpoint behavior and limits.
- [Prometheus metrics](METRICS.md) and [monitoring and alerting](MONITORING-ALERTS.md)
  — scrape setup and alert examples.
- [Grafana dashboard](GRAFANA.md) and [dashboard JSON](grafana-dashboard.json)
  — importable starter visualization.
- [Logging](LOGGING.md) — log formats, levels, and retention considerations.
- [Performance tuning](PERFORMANCE.md) — choose settings for the measured path
  and workload.
- [Performance documentation index](../testing/PERFORMANCE_INDEX.md) — find
  tuning guides, benchmark methods, and recorded results.
- [Resource planning](RESOURCES.md) — configuration-derived resource budgets
  and sizing considerations.
- [Service level objectives](SLO.md) — operator-defined availability goals.

## Project references

- [Project positioning](../POSITIONING.md) — scope and operating principles.
- [Protocol specification](../PROTOCOL.md) — wire-level contract.
- [Architecture](../ARCHITECTURE.md) — component and data-flow overview.
