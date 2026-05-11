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
connections, handshake success/failure counters, stream counters, and byte
totals.

`/status` and `/connections` also include runtime state: tunnel state,
reconnect count, consecutive failures, recent errors, egress policy version,
process start time, and last config apply time.

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
