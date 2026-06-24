# Admin And Metrics

Espejismo can expose a small HTTP admin endpoint from either binary. It is
disabled by default.

```toml
[admin]
listen = "127.0.0.1:9090"
token = "change-me-admin-token"
```

Endpoints:

- `GET /healthz`: health probe.
- `GET /status`: JSON status snapshot.
- `GET /connections`: metrics plus runtime tunnel state for troubleshooting.
- `GET /metrics`: Prometheus-style text metrics.
- `POST /reload`: reload the original `--config` or `--config-base64` source.
- `POST /apply`: apply a TOML config supplied as the request body.

Authentication:

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

`/status` and `/connections` also include runtime state: tunnel state,
reconnect count, consecutive failures, recent errors, egress policy version,
process start time, last config apply time, lane RTT samples, per-lane session
age, active streams, streams opened, stream open failures, last activity time,
last error time, and per-lane byte totals.

`/metrics` exposes the lane snapshot as Prometheus-style series when local
tunnel lanes exist:

```text
espejismo_tunnel_lane_active_streams{role="local",lane_id="0",lane_kind="bulk",state="connected"} 2
espejismo_tunnel_lane_streams_opened{role="local",lane_id="0",lane_kind="bulk",state="connected"} 42
espejismo_tunnel_lane_stream_open_failures{role="local",lane_id="0",lane_kind="bulk",state="connected"} 0
espejismo_tunnel_lane_bytes_client_to_remote{role="local",lane_id="0",lane_kind="bulk",state="connected"} 1048576
espejismo_tunnel_lane_bytes_remote_to_client{role="local",lane_id="0",lane_kind="bulk",state="connected"} 2048
espejismo_tunnel_lane_last_open_latency_ms{role="local",lane_id="0",lane_kind="bulk",state="connected"} 158
```

Runtime apply updates new tunnels and newly opened logical streams. A restart is
still required for process-owned resources such as listener sockets,
`admin.listen`, TUN device ownership, and log file handles.

Remote runtime-managed settings include users, quotas, bandwidth limits, egress
policy, fallback behavior, handshake timing, frame shaping, and stream limits.
Local runtime-managed settings include `local.server`, local proxy auth,
TCP/pacing/obfuscation knobs, mux mode, and `local.tunnel_pool`; applying those
settings rebuilds the tunnel pool without restarting the local process.
Existing established streams keep their current resources until they naturally
close.

Keep admin listeners bound to loopback unless they sit behind a trusted local
firewall or service manager.
