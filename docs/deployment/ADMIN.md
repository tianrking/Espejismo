# Admin And Metrics

Espejismo can expose a small read-only HTTP admin endpoint from either binary.
It is disabled by default.

```toml
[admin]
listen = "127.0.0.1:9090"
token = "change-me-admin-token"
```

Endpoints:

- `GET /healthz`: health probe.
- `GET /status`: JSON status snapshot.
- `GET /metrics`: Prometheus-style text metrics.

Authentication:

```bash
curl -H 'Authorization: Bearer change-me-admin-token' http://127.0.0.1:9090/status
curl -H 'X-Espejismo-Admin-Token: change-me-admin-token' http://127.0.0.1:9090/metrics
```

Metrics include active physical connections, active logical streams, accepted
connections, handshake success/failure counters, stream counters, and byte
totals.

Keep admin listeners bound to loopback unless they sit behind a trusted local
firewall or service manager.
