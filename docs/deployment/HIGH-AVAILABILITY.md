# High-Availability Deployments

Espejismo can run behind operator-managed endpoint failover or a TCP load
balancer. The binaries do not provide clustering, replicated session state,
automatic peer discovery, or coordinated failover. Keep the deployment model
small: one client configuration points at one stable public endpoint, and
infrastructure directs that endpoint to one or more independently operated
remote processes.

## What failover does

Each physical tunnel belongs to the remote process that accepted it. A load
balancer change, host failure, or process restart does not move that connection
or its streams to another remote. The client can establish a new tunnel after
the endpoint becomes reachable; applications may need to retry interrupted
requests or transfers. See [Shutdown and Connection Draining](SHUTDOWN.md).

The optional `GET /healthz` endpoint reports process liveness only. It does
not test the tunnel handshake, egress, or an application request. Use it to
remove a process that cannot answer HTTP probes, and separately monitor
representative client connectivity and proxy traffic. See [Health Checks](HEALTHCHECK.md)
and [Monitoring and Alerting](MONITORING-ALERTS.md).

## Active/passive with a stable endpoint

Run one serving remote and keep a second remote configured and ready. Publish
one stable address through a floating IP, a health-checked TCP load balancer,
or operator-managed DNS failover. Point each client's `local.server` at that
address. Only the active target needs to receive new connections during normal
operation. On failure, move the endpoint to the standby and allow clients to
reconnect.

DNS failover depends on resolver and client caching behavior, so the configured
TTL is not a guaranteed recovery time. A floating IP or load balancer usually
gives the operator a clearer health and cutover control. In either case,
existing connections to the failed target are lost.

## Active/active TCP load balancing

Place multiple remotes behind a TCP pass-through load balancer and configure
the client to use the single public address. Each new TCP connection can land
on any healthy remote. Configure the balancer to stop assigning new connections
to unhealthy targets; established connections remain attached to their target
and can still be interrupted if that target is stopped or fails. Espejismo does
not request a readiness transition or drain existing streams on shutdown.

Use a TCP listener and pass-through for the default TCP underlay. Confirm that
the balancer's idle timeout and connection limits accommodate the deployment's
tunnel behavior. Do not terminate TLS on the balancer: Espejismo's protocol is
not TLS. WebSocket and HTTP/2 underlays need a proxy that preserves their
configured routing details and supports their long-lived connections; validate
that path separately before relying on it for failover.

## Keep remote instances equivalent

Every remote target must be able to authenticate the same clients and provide
the intended service. Keep these items consistent across targets:

- Compatible Espejismo versions and shared protocol settings. Follow
  [Version Compatibility](VERSION-COMPATIBILITY.md).
- The same relevant `shared.psk` or matching `remote.users` names and PSKs.
- Equivalent egress policy, DNS behavior, destination reachability, and
  resource limits for the service being balanced.
- Firewall rules and listener ports that match the balancer's target checks.
- Matching underlay settings when using WebSocket or HTTP/2.

Keep admin listeners private. If the load balancer probes `/healthz`, allow its
probe path to reach the admin listener with host or network policy and a
non-loopback bind only where required. The health route itself is unauthenticated;
do not publish the admin port to the public network. `/status` and `/metrics`
remain authenticated operational endpoints. See [Admin and Metrics](ADMIN.md).

Port hopping selects a port from the client's configured list, while a
load-balancer endpoint may route only a different set of addresses and ports.
For a basic HA deployment, leave `shared.port_hopping.enabled` disabled. If it
is required, make every selected port reachable through the same equivalent
healthy target set and test failover at each port.

## Failover checklist

1. Pin compatible binaries and back up each remote's config and secrets.
2. Validate each config with `--check-config`, start each remote, and verify
   its listener and firewall from the intended client network.
3. Confirm a real `--probe-server` and a representative proxy request through
   the stable public endpoint. A successful `/healthz` alone is insufficient.
4. Remove one target from new traffic or move the active/passive endpoint.
   Confirm new client connections work, and record how existing connections
   are interrupted.
5. Restore the target, check logs and metrics, and document the observed
   recovery time for the actual DNS, balancer, and client settings.

For updates, withdraw one remote target from new connections before replacing
its binary, then restore it only after its version, health probe, and proxy
traffic are verified. This reduces the number of new connections sent to a
restarting instance; it does not drain its active streams. See the
[deployment runbook](RUNBOOK.md) and [shutdown behavior](SHUTDOWN.md).
