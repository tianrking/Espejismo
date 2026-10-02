# Admin And Metrics

Espejismo can expose a small HTTP admin endpoint from either binary. It is
disabled by default.

```toml
[admin]
listen = "127.0.0.1:9090"
token = "change-me-admin-token"
```

Endpoints:

- `GET /healthz`: unauthenticated liveness probe returning only `ok`; it does
  not report tunnel readiness or runtime details.
- `GET /status`: JSON status snapshot.
- `GET /connections`: metrics plus runtime tunnel state for troubleshooting.
- `GET /metrics`: Prometheus-style text metrics.
- `POST /reload`: reload the original `--config` or `--config-base64` source.
- `POST /apply`: apply a TOML config supplied as the request body.

Authentication:

`GET /healthz` is the only unauthenticated route, so a load balancer can probe
the process without an admin credential. All other routes below require the
configured token.

## Configuration reload

Configuration is not watched automatically, and neither binary handles
`SIGHUP`. Use the authenticated admin endpoint to request an update:

- `POST /reload` rereads the original `--config` file or `--config-base64`
  value. It is unavailable if the process was started without either source.
- `POST /apply` parses the TOML request body as a candidate config. It does not
  replace the source used by a later `/reload`.

Both actions apply the startup CLI overrides again, so an overridden value in
the config file or `/apply` body does not supersede the command line. The
client also reapplies its startup profile import. The candidate is parsed and
built before runtime settings are replaced; on failure, the current settings
remain active. A successful response means the in-memory settings were
accepted, not that every process resource was recreated. The response's
`restart_required_for` field names primary restart cases; use the table below
for the full set of startup-captured settings. `/status` exposes the last
successful apply timestamp.

| Process | Updated by reload/apply | Restart required for |
| --- | --- | --- |
| Remote | Users and handshake credentials/policy; per-user quotas and bandwidth limits; egress and fallback policy; handshake/reject/cold-start timing; frame shaping, underlay and mux settings; idle and logical-stream limits. These are the settings assembled into the remote runtime policy. | `remote.listen`, `admin.listen`, `logging.file`, shared TCP socket options, port hopping, tunnel buffer, replay window, tarpit limits, and process-wide physical-connection/logical-stream semaphore capacities. These are captured by the running listener/process. |
| Local | `local.server`, proxy authentication, handshake settings, TCP options, pacing, obfuscation/frame shaping, underlay and mux settings, tunnel pool, tunnel buffer, HTTP bulk threshold, and idle timeout. The tunnel manager is replaced for subsequent tunnel work. | SOCKS5/HTTP listen addresses, TUN device/route/DNS ownership and settings, `admin.listen`, and `logging.file`. These belong to already-created listeners, the active TUN setup, or open log handles. |

For either process, newly created tunnels and newly opened logical streams use
the updated runtime settings. Established streams keep their existing
resources and settings until they close; an update does not renegotiate an
active stream. Settings supplied only through a changed config source take
effect after the next successful reload/apply. A restart is needed for the
process-owned settings above. See [Configuration](CONFIG.md) for each field's
meaning and [Runbook](RUNBOOK.md) for upgrade/restart procedure.

```bash
curl -H 'Authorization: Bearer change-me-admin-token' http://127.0.0.1:9090/status
curl -H 'Authorization: Bearer change-me-admin-token' http://127.0.0.1:9090/connections
curl -H 'X-Espejismo-Admin-Token: change-me-admin-token' http://127.0.0.1:9090/metrics
curl -X POST -H 'Authorization: Bearer change-me-admin-token' http://127.0.0.1:9090/reload
curl -X POST -H 'Authorization: Bearer change-me-admin-token' --data-binary @espejismo.toml http://127.0.0.1:9090/apply
```

Metrics include active physical connections, active logical streams, accepted
connections, handshake success/failure counters, stream counters, byte totals,
egress deny counters, stream failure reason counters, session rotation counters,
frame key-update counters, and local tunnel lane counters.
See [Prometheus Metrics](METRICS.md) for the complete metric catalog, types,
label definitions, units, scope, and example queries.

`/status` and `/connections` also include runtime state: tunnel state,
reconnect count, consecutive failures, recent errors, egress policy version,
process start time, last config apply time, lane RTT samples, per-lane session
age, active streams, pending stream opens, streams opened, stream open failures,
last activity time, last error time, and per-lane byte totals.

`/metrics` exposes the lane snapshot as Prometheus-style series when local
tunnel lanes exist:

```text
espejismo_tunnel_lane_active_streams{role="local",lane_id="0",lane_kind="bulk",state="connected"} 2
espejismo_tunnel_lane_pending_stream_opens{role="local",lane_id="0",lane_kind="bulk",state="connected"} 0
espejismo_tunnel_lane_streams_opened{role="local",lane_id="0",lane_kind="bulk",state="connected"} 42
espejismo_tunnel_lane_stream_open_failures{role="local",lane_id="0",lane_kind="bulk",state="connected"} 0
espejismo_tunnel_lane_bytes_client_to_remote{role="local",lane_id="0",lane_kind="bulk",state="connected"} 1048576
espejismo_tunnel_lane_bytes_remote_to_client{role="local",lane_id="0",lane_kind="bulk",state="connected"} 2048
espejismo_tunnel_lane_last_open_latency_ms{role="local",lane_id="0",lane_kind="bulk",state="connected"} 158
```

Keep admin listeners bound to loopback unless they sit behind a trusted local
firewall or service manager.
