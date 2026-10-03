# Grafana Dashboard

`grafana-dashboard.json` is an importable starter dashboard for Prometheus
scrapes of Espejismo's authenticated admin `/metrics` endpoint. First
configure Prometheus as a Grafana data source, then use **Dashboards → New →
Import** and upload the JSON file. Select the Prometheus data source when
Grafana asks to map the dashboard input. The dashboard assumes the scrape job
is named `espejismo`; edit the Job variable query if you use another name.

The dashboard includes current physical connections and streams, tunnel payload
throughput, handshake and stream rates, and local lane mux RTT. Use the Job and
Instance selectors to narrow the view. The panels use five-minute `rate()`
windows, so short-lived changes can appear smoothed and counters need enough
scrape history before rates are available.

Metrics are process-local. Scrape both tunnel peers and select an instance to
inspect each separately. Both peers observe the same tunneled payload, so do
not add their byte rates together as distinct traffic. Byte rates describe
tunnel payload, not wire utilization. Lane RTT exists only on the local client
and appears after a sample has been observed. See [Prometheus Metrics](METRICS.md)
for metric definitions and [Monitoring And Alerting](MONITORING-ALERTS.md) for
scrape setup and operational guidance.
