# High-availability deployment documentation

## Findings and plan

- The client configuration has one `local.server` endpoint. A stable endpoint
  supplied by operator-managed DNS, a floating address, or a TCP load balancer
  fits the existing small operations model without changing the product.
- Remotes accept independent tunnels; the project has no shared session state,
  cluster membership, or coordinated process drain. Failover can restore new
  connections, but established streams can be interrupted.
- `/healthz` is unauthenticated liveness only. It cannot be presented as
  tunnel readiness; actual client probes and representative traffic are needed
  for service verification.
- A TCP pass-through load balancer is the clearest default-underlay topology.
  WebSocket/HTTP2 routing and port hopping require the operator to preserve
  matching routing/port behavior and validate the path explicitly.

## Change and expected benefit

Added `docs/deployment/HIGH-AVAILABILITY.md` describing active/passive and
active/active stable-endpoint patterns, instance consistency, health checks,
failover limitations, and an operational test/update checklist. The expected
benefit is fewer configuration and recovery assumptions during HA setup; no
availability percentage is promised because recovery depends on external DNS,
load-balancer, network, and application retry behavior. There are no runtime or
protocol changes.

## Verification

Documentation-only review: checked statements against `local.server` and
underlay/port-hopping config documentation, `HEALTHCHECK.md`, `ADMIN.md`,
`SHUTDOWN.md`, `RUNBOOK.md`, and `POSITIONING.md`. No build or tests are
applicable. No measured availability improvement is claimed.
