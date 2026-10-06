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

`GET /healthz` is always unauthenticated, so a load balancer can probe the
process without an admin credential. When `admin.token` is configured, every
other route requires that token in either the `Authorization: Bearer` header
or the `X-Espejismo-Admin-Token` header. If no token is configured, the server
does not authenticate requests; configuration validation requires a token for
non-loopback admin listeners. Keep an unauthenticated listener bound to
loopback. With a token configured, missing or invalid credentials receive
HTTP 401 before request bodies are read or administrative actions are run.

For probe behavior and container/orchestrator examples, see
[Health Checks](HEALTHCHECK.md).

## Configuration reload

Configuration is not watched automatically. The remote binary reloads its
original config source on Unix when it receives `SIGHUP`; the local binary does
not handle `SIGHUP`. Either binary can also use the authenticated admin endpoint
to request an update:

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
| Remote | `remote.users` (including each user's `name`, `psk`, `quota.*`, and `bandwidth.*`); `remote.egress.*`; `remote.fallback_http.*`; `remote.handshake_timeout_ms`, `remote.reject_delay_ms`, `remote.cold_start_delay_ms`; `shared.psk`, `shared.clock_skew_secs`, `shared.puzzle_bits`, `shared.handshake_window.*`, `shared.max_padding`, `shared.jitter_ms`, `shared.padding_chance_percent`, `shared.backpressure_*`, `shared.key_update_frames`, `shared.tcp.heartbeat_secs`, `shared.obfuscation.*`, `shared.stealth.*`, `shared.stealth_shaper.*`, `shared.pacing.*`, `shared.underlay.*`, `shared.mux.*`, `shared.max_streams`, and `shared.idle_timeout_secs`. These are the values assembled into `RemoteSettings` and used by new tunnels/streams. | `remote.listen`; `admin.listen` and `admin.token`; `logging.*`; `remote.replay_window_secs`, `remote.max_handshake_padding`, `remote.tarpit_*`; `shared.tcp` socket options (except `heartbeat_secs`); `shared.port_hopping.*`, `shared.tunnel_buffer`, `shared.max_physical_connections`. These are captured by the running listener/process, admin service, or logging subscriber. |
| Local | `local.server`, `local.auth`; `local.handshake_padding`, `local.http_bulk_threshold_bytes`; `local.tunnel_pool.*`; `shared.psk`, `shared.clock_skew_secs`, `shared.puzzle_bits`, `shared.handshake_window.*`, `shared.max_padding`, `shared.jitter_ms`, `shared.padding_chance_percent`, `shared.backpressure_*`, `shared.key_update_frames`, `shared.tcp.*`, `shared.pacing.*`, `shared.obfuscation.*`, `shared.stealth.*`, `shared.stealth_shaper.*`, `shared.underlay.*`, `shared.mux.*`, `shared.tunnel_buffer`, and `shared.idle_timeout_secs`. Reload/apply replaces the tunnel manager, so the new runtime settings are used for subsequent tunnel work. | `local.socks5_listen`, `local.http_listen`, `local.tun.*`, `admin.listen` and `admin.token`, and `logging.*`. These belong to already-created listeners, active TUN setup, admin service, or logging subscriber. |

The wildcard notation above includes every child key in that TOML table. A listed
setting is accepted into runtime state; it does not reconfigure an established
physical tunnel or logical stream. In particular, local TCP and mux settings
apply as new tunnels are created, while streams already using a tunnel keep
their existing resources. Startup CLI overrides (and the local startup profile
import) continue to take precedence over config values during reload/apply.

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
label definitions, units, scope, and example queries. For Prometheus scrape
configuration and starter alert rules, see [Monitoring And Alerting](MONITORING-ALERTS.md).
The scrape guide uses Prometheus' `bearer_token_file`, which sends the
`Authorization: Bearer` header accepted by this endpoint.

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
