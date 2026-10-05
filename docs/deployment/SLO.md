# Service Level Objectives

Espejismo does not publish a hosted service commitment or a universal
availability guarantee. These objectives are an operator starting point for a
self-managed tunnel. Set targets from the workload, route, and maintenance
model you can measure; record the chosen targets alongside the deployment
runbook.

An SLO is an internal reliability goal over a stated window. It is not a
service level agreement, and passing a process health check alone does not
mean proxy traffic is working. Espejismo remains a small, operator-managed
encrypted tunnel; these recommendations use its existing endpoints and do not
require a monitoring service from the project.

## Suggested objectives

| Signal | Suggested starting objective | Measurement |
| --- | --- | --- |
| End-to-end tunnel availability | At least 99.0% of scheduled checks succeed over a rolling 30-day window | From the client host, make a small authenticated proxy request through the configured SOCKS5 or HTTP proxy to an operator-controlled probe destination. Count each scheduled check once; a timeout or unsuccessful response is a failure. |
| Interactive request latency | 95th percentile of probe latency is no more than the direct-path p95 plus 100 ms over the same window | Measure the same request directly and through the proxy from the same client and destination. Keep the request and sampling schedule consistent. This is a suggested budget, not a product guarantee. |
| Stream establishment | Fewer than 1% of attempted sampled proxy requests fail to establish a usable stream over 30 days, with at least 100 attempts before treating the ratio as representative | Divide failed attempts by all attempted requests, including failures. The client-side synthetic request is the source of truth. Server counters can help diagnose failures but do not include all client-side observations. |
| Recovery after an operator change | Restore a representative probe within the deployment's declared recovery time objective (start with 30 minutes) after a failed upgrade or configuration change | Exercise the documented rollback procedure during a maintenance window and record detection, rollback, and recovery times. |

These values are example starting points, not measured Espejismo results.
Choose a different target where the path, destination, or maintenance needs
justify it. Do not turn the throughput figures from a benchmark into an
availability or latency promise.

## Measurement and interpretation

- Run the end-to-end probe from the client side at a stable interval (for
  example, once per minute). Use an operator-controlled destination that
  returns a small known response, and avoid relying on a third-party website
  whose outage would be attributed to the tunnel. Keep a record of scheduled,
  successful, and failed attempts so the denominator is explicit.
- Define the scheduled window, maintenance exclusions, probe timeout, and
  treatment of client-host outages before calculating compliance. Report both
  excluded time and observed failures; do not silently remove incidents.
- A successful `/healthz` means only that the process answered HTTP. It does
  not prove peer reachability or working proxy traffic. Use the synthetic
  proxied request for end-to-end availability and `/status` or metrics to
  diagnose it. See [Health Checks](HEALTHCHECK.md) and
  [Monitoring And Alerting](MONITORING-ALERTS.md).
- The current metrics expose counters, lane gauges, and the latest observed
  stream-open latency. They do not expose request latency histograms or a
  built-in end-to-end success ratio. Prometheus metrics alone therefore cannot
  calculate the latency percentile or end-to-end availability objectives
  above; collect probe results externally. Avoid deriving a percentile from
  `espejismo_tunnel_lane_last_open_latency_ms`, which is only a last-value
  gauge.
- Tunnel byte counters count payload, not wire bytes or link capacity. If a
  deployment has a throughput objective, state the workload, direction,
  profile, path RTT, and direct-path baseline, then compare repeated runs with
  `scripts/bench-throughput.sh`. There is no universal minimum throughput
  objective for different routes and profiles.

## Error budget and response

For a 30-day window, a 99.0% availability target allows 1% of scheduled checks
to fail. With checks every minute and no exclusions, that is about 432 failed
checks, or 7 hours 12 minutes of equivalent time. This is a budgeting
calculation, not a prediction of downtime. Review incidents and probe
coverage monthly. If the budget is being consumed quickly, prioritize
diagnosing the recurring failure mode and defer risky changes until service
stabilizes. Use the
[Deployment Runbook](RUNBOOK.md) for upgrade and rollback steps, and the
[Monitoring And Alerting](MONITORING-ALERTS.md) guide for existing scrape and
failure alerts.

Keep the operational response proportional to the deployment: one client and
one remote may be enough for a personal tunnel. SLO measurement does not
require public admin endpoints, a new exporter, or protocol camouflage. Keep
admin access restricted as described in the monitoring guide.
