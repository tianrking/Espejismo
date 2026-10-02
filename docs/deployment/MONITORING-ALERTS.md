# Monitoring And Alerting

This guide shows a small Prometheus setup for the existing admin endpoints.
It uses the existing metrics and does not require a separate exporter. For
metric definitions and label semantics, see [Prometheus Metrics](METRICS.md);
for admin listener setup and endpoint authentication, see [Admin](ADMIN.md).

## Enable a scrape endpoint

Configure the admin listener on loopback on each host. Use a unique, strong
token and keep the config file readable only by the service account and
administrators:

```toml
[admin]
listen = "127.0.0.1:9090"
token = "replace-with-a-long-random-token"
```

The listener is disabled by default. If Prometheus runs on another host, do
not expose this endpoint directly to the public network. Use a private network
and firewall or a trusted authenticated proxy, and require a token. The
unauthenticated `/healthz` endpoint reports process liveness only; it does not
prove that a tunnel is connected or traffic can pass.

## Prometheus scrape configuration

Prometheus supports a bearer token file for the `Authorization` header. Store
the token in a file accessible only to the Prometheus process (one line, no
`Bearer` prefix), for example `/etc/prometheus/espejismo-admin.token`.

```yaml
scrape_configs:
  - job_name: espejismo
    metrics_path: /metrics
    scrape_interval: 30s
    static_configs:
      - targets:
          - 127.0.0.1:9090 # local process on this host
        labels:
          role: remote
          instance: remote-1
    bearer_token_file: /etc/prometheus/espejismo-admin.token
```

Run an equivalent job for each process. A local client should use a distinct
instance label and a `role: local` target label. The target label is useful in
dashboards but is independent of the metric's own `role` label. If Prometheus
is not on the same host, replace the loopback target only after restricting
network access to the monitoring path.

Check `up{job="espejismo"}` after reloading Prometheus. A value of `1` means
the scrape succeeded; `0` usually indicates a listener, network, token, or
configuration problem. A missing series can mean the target is not configured
or has not yet been scraped.

## Starter alert rules

Save these examples as `espejismo-alerts.yml` and load them through a
Prometheus `rule_files` entry. The thresholds and `for` durations are starting
points; tune them against normal traffic and expected maintenance windows.

```yaml
groups:
  - name: espejismo
    rules:
      - alert: EspejismoScrapeFailed
        expr: up{job="espejismo"} == 0
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "Espejismo metrics endpoint is unavailable"
          description: "Prometheus cannot scrape {{ $labels.instance }}. Check the process, admin listener, network path, and bearer token."

      - alert: EspejismoHandshakeFailures
        expr: |
          sum by (instance, role) (increase(espejismo_handshake_failure_total[10m])) > 5
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Espejismo handshakes are failing"
          description: "More than five handshake failures were recorded in each recent 10-minute window for {{ $labels.instance }}. Check peer reachability, version compatibility, and shared credentials."

      - alert: EspejismoStreamFailures
        expr: |
          sum by (instance, role) (increase(espejismo_stream_failed_total[10m])) > 0
          and
          sum by (instance, role) (increase(espejismo_stream_failed_total[10m]))
            / clamp_min(sum by (instance, role) (increase(espejismo_stream_opened_total[10m])), 1) > 0.05
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Espejismo stream failure rate is elevated"
          description: "Stream failures exceed 5% of opened streams over recent windows for {{ $labels.instance }}. Inspect failure reasons and service logs."
```

The handshake rule detects a sustained count, not a failure percentage; a
busy deployment may need a higher threshold. The stream rule requires at least
one failure and compares failures with opened streams. Short windows and low
traffic can produce noisy ratios, so adjust its window or `for` duration for
your workload. Metric counters reset when a process restarts; `increase()`
handles counter resets for these interval checks.

These metrics are process-local. Configure scrapes for both tunnel peers and
compare them by `instance` and the exported `role`; do not sum local and remote
byte counters as if they represented distinct traffic. Byte metrics count
tunneled payload, not wire utilization. The exporter has no retransmission
counter or live mux-window occupancy metric; alerting on those conditions
requires other host/network telemetry.

## Operational response

- **Scrape failed:** verify the service is running, the admin address is
  reachable from Prometheus, and its token file matches the configured token.
- **Handshake failures:** compare both peers' logs and configured credentials,
  then check network reachability and the [version compatibility policy](VERSION-COMPATIBILITY.md).
- **Stream failures:** inspect
  `rate(espejismo_stream_failure_reason_total[5m])`, service logs, and recent
  configuration changes. On the local client, use `/status` or `/connections`
  to inspect lane state and reconnect details.

Alert delivery is handled by the operator's existing Alertmanager or other
notification system; Espejismo does not send alerts itself.
