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

## Configure the dashboard

The Job selector defaults to `espejismo`; change its query and default in the
dashboard variables if your Prometheus `job_name` differs. Instance choices
come from `up` series for the selected job. The dashboard uses Prometheus's
`instance` label to separate processes, so assign stable, distinct instance
labels to the local client and remote server. The scrape target's `role` label
is not used by these panels.

The stat panels show the latest active connection and stream gauges. The
throughput panel groups byte counter rates by instance and direction; these
are payload bytes per second, not interface traffic. Handshake and stream
panels show successful/failed handshakes and opened/failed streams per second.
The lane RTT panel is client-local, split by lane ID and kind, and may be empty
until the client has recorded a sample. These are diagnostic views, not
availability or user-facing service-level indicators; configure alert rules
separately as described in [Monitoring And Alerting](MONITORING-ALERTS.md).

To adapt the dashboard, adjust the Job variable to match scrape labels, or
edit a panel's PromQL and legend in Grafana. Keep the `instance` grouping when
comparing processes. If you change the five-minute rate window, allow enough
scrape samples for the new range before expecting a series. The dashboard
contains no scrape credentials: configure bearer-token access on Prometheus as
described in the monitoring guide, and never put the admin token in a panel
query or dashboard variable.

Metrics are process-local. Scrape both tunnel peers and select an instance to
inspect each separately. Both peers observe the same tunneled payload, so do
not add their byte rates together as distinct traffic. Byte rates describe
tunnel payload, not wire utilization. Lane RTT exists only on the local client
and appears after a sample has been observed. See [Prometheus Metrics](METRICS.md)
for metric definitions and [Monitoring And Alerting](MONITORING-ALERTS.md) for
scrape setup and operational guidance.
