# Prometheus Metrics

The authenticated admin endpoint `GET /metrics` returns Prometheus text
exposition. It combines process counters from the shared metrics collector with
runtime lane metrics on the local client. The endpoint is disabled with the
admin listener; see [Admin and Metrics](ADMIN.md) for setup and authentication.
For a ready-to-adapt Prometheus `scrape_configs` example using a protected
token file, see [Monitoring And Alerting](MONITORING-ALERTS.md).

You can check the endpoint locally with the same bearer token Prometheus uses:

```bash
curl -H 'Authorization: Bearer change-me-admin-token' \
  http://127.0.0.1:9090/metrics
```

The sample token must match `admin.token` in the process configuration. Keep
the admin listener on loopback or a restricted monitoring network; the metrics
route is authenticated and is not a public exporter.

All metric names below use the `espejismo_` prefix. Process counters are
cumulative since process start and reset on restart; use `rate()` or
`increase()` for rates and interval totals. Lane counters belong to the current
local tunnel manager and reset when that manager is replaced (for example,
after a configuration apply) or the process restarts. Gauges describe the
current scrape-time value. Metrics are local observations, not a view
aggregated across both tunnel peers. The local and remote processes observe the
same tunneled payload from opposite ends. Scrape both peers when diagnosing a
path, but do not sum their byte counters to estimate unique traffic; that
counts each payload byte twice. Choose one role for traffic totals, or graph
each role separately to compare the observations.

## Labels

| Label | Meaning |
| --- | --- |
| `role` | Process role: `local` for the client and `remote` for the server. Present on every series. |
| `user` | Authenticated remote-side user. Present on per-user series, only emitted by the remote. Unknown excess users are aggregated as `other`. |
| `reason` | Sanitized stream failure class. Present on failure-reason series from either role. Excess distinct reasons are aggregated as `other`. |
| `lane_id` | Local tunnel lane identifier. Present on lane series. |
| `lane_kind` | Configured lane class, such as `bulk` or `interactive`. Present on lane series. |
| `state` | Lane runtime state at scrape time. Present on lane series. |

Per-user labels are bounded per process to 128 distinct `user` values,
including `user="other"`. Each admitted user value has four metric series:
handshake successes, streams opened, and payload bytes in each of the two
tunnel directions. Excess users share the `other` value across all four
series. Per-reason labels are bounded to 32 values, including the `other`
bucket. Lane labels represent the
configured local pool and current runtime state; do not treat lane IDs or
states as stable across restarts or configuration changes. No peer address is
exported as a label.

## Process metrics

These series are emitted by both roles. They have only the `role` label.

| Metric | Type | Meaning |
| --- | --- | --- |
| `espejismo_active_physical_connections` | Gauge | Currently active physical tunnel connections, including connections in handshake; on the remote this counts the accepted tunnel underlay after protocol/fallback routing. |
| `espejismo_active_streams` | Gauge | Currently active logical streams handled by the process. The remote counts streams after the per-user stream limit admits them. |
| `espejismo_accepted_connections_total` | Counter | Connections accepted by the process listener: tunnel connections on the remote, SOCKS5/HTTP proxy connections on the local. The remote increments after its global connection limit admits the connection. |
| `espejismo_handshake_success_total` | Counter | Successful tunnel handshakes. |
| `espejismo_handshake_failure_total` | Counter | Failed tunnel handshakes. |
| `espejismo_stream_opened_total` | Counter | Logical streams opened by the process. |
| `espejismo_stream_failed_total` | Counter | Logical streams that failed. |
| `espejismo_egress_denied_total` | Counter | Remote egress requests denied by policy. Zero on the local role. |
| `espejismo_session_rotations_total` | Counter | Client tunnel session/lane rotations. Zero on the remote role. |
| `espejismo_key_updates_total` | Counter | Frame encryption key updates. |
| `espejismo_bytes_client_to_remote_total` | Counter | Tunnel payload bytes from local client toward remote, observed by this process. |
| `espejismo_bytes_remote_to_client_total` | Counter | Tunnel payload bytes from remote toward local client, observed by this process. |

Byte counters count tunneled payload bytes at the application tunnel layer, not
wire bytes, framing overhead, retransmissions, or link utilization. Calculate
observed bytes per second with, for example:

```promql
rate(espejismo_bytes_client_to_remote_total[5m])
rate(espejismo_bytes_remote_to_client_total[5m])
```

## Remote per-user metrics

These metrics are emitted by the remote process and carry `role` and `user`.
The counters are scoped to the authenticated user.

| Metric | Type | Meaning |
| --- | --- | --- |
| `espejismo_user_handshake_success_total` | Counter | Successful handshakes for the user. |
| `espejismo_user_stream_opened_total` | Counter | Logical streams opened for the user. |
| `espejismo_user_bytes_client_to_remote_total` | Counter | Payload bytes forwarded toward remote egress for the user. |
| `espejismo_user_bytes_remote_to_client_total` | Counter | Payload bytes forwarded back to the user. |

## Stream failure reason metrics

`espejismo_stream_failure_reason_total` is a counter with `role` and `reason`
labels. It counts stream failures grouped by a stable failure class. The remote
records stream-handler failures; the local client records lane-connect and mux
open failures. Reason values are sanitized to ASCII letters, digits, `_`, or
`-`, and limited to 64 characters. Distinct excess values are counted under
`reason="other"`.

## Local tunnel lane metrics

These metrics are emitted only by the local client and carry `role`,
`lane_id`, `lane_kind`, and `state`. Byte and stream counters are lane-scoped
and last for the lifetime of the current tunnel manager; reconnecting a lane
does not reset them, while replacing the manager does. Lane samples appear once
the runtime has published that lane (they may be absent during startup).

| Metric | Type | Meaning |
| --- | --- | --- |
| `espejismo_tunnel_lane_active_streams` | Gauge | Active logical streams on the lane. |
| `espejismo_tunnel_lane_streams_opened` | Counter | Streams opened through the lane. |
| `espejismo_tunnel_lane_pending_stream_opens` | Gauge | Stream opens currently reserved on the lane. |
| `espejismo_tunnel_lane_stream_open_failures` | Counter | Failed stream opens on the lane. |
| `espejismo_tunnel_lane_bytes_client_to_remote` | Counter | Cumulative client-to-remote payload bytes on the lane. |
| `espejismo_tunnel_lane_bytes_remote_to_client` | Counter | Cumulative remote-to-client payload bytes on the lane. |
| `espejismo_tunnel_lane_recent_client_to_remote_bps` | Gauge | Recent EWMA client-to-remote throughput, in bytes per second. |
| `espejismo_tunnel_lane_recent_remote_to_client_bps` | Gauge | Recent EWMA remote-to-client throughput, in bytes per second. |
| `espejismo_tunnel_lane_adaptive_score` | Gauge | Current adaptive lane-selection score; lower scores are preferred. This is an internal ranking value, not a latency or throughput unit. |
| `espejismo_tunnel_lane_reconnect_count` | Counter | Reconnects recorded for the lane. |
| `espejismo_tunnel_lane_last_open_latency_ms` | Gauge | Most recent stream-open latency, in milliseconds. |
| `espejismo_tunnel_lane_last_mux_rtt_ms` | Gauge | Most recent mux ping RTT, in milliseconds; absent until observed. |
| `espejismo_tunnel_lane_session_age_secs` | Gauge | Current lane session age, in seconds; absent when no session is active. |
| `espejismo_tunnel_lane_last_activity_unix_secs` | Gauge | Unix timestamp in seconds of the lane's last activity; absent until activity is observed. |

Lane byte counters are payload totals. The recent throughput gauges are
smoothed runtime estimates; they are not scrape-window rates. Last-observed
latency and activity values remain gauges and should not be interpreted as
histograms or event counters. The exporter does not currently expose transport
retransmissions or live mux window occupancy.
