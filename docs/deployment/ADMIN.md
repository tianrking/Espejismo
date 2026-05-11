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
- `GET /metrics`: Prometheus-style text metrics.
- `POST /reload`: remote only, reload the original `--config` or
  `--config-base64` source.
- `POST /apply`: remote only, apply a TOML config supplied as the request body.

Authentication:

```bash
curl -H 'Authorization: Bearer change-me-admin-token' http://127.0.0.1:9090/status
curl -H 'X-Espejismo-Admin-Token: change-me-admin-token' http://127.0.0.1:9090/metrics
curl -X POST -H 'Authorization: Bearer change-me-admin-token' http://127.0.0.1:9090/reload
curl -X POST -H 'Authorization: Bearer change-me-admin-token' --data-binary @espejismo.toml http://127.0.0.1:9090/apply
```

Metrics include active physical connections, active logical streams, accepted
connections, handshake success/failure counters, stream counters, and byte
totals.

Runtime apply updates new remote tunnels and newly opened logical streams. A
restart is still required for process-owned resources such as `remote.listen`,
`admin.listen`, and log file handles.

Keep admin listeners bound to loopback unless they sit behind a trusted local
firewall or service manager.
