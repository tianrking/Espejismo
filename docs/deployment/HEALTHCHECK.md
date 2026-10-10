# Health Checks

Espejismo provides one HTTP liveness endpoint on the optional admin listener:
`GET /healthz`. It responds with HTTP `200` and the plain text body `ok` plus
a newline. It is available on both `espejismo-local` and `espejismo-remote`
when `[admin].listen` is configured. The listener is disabled by default.
The endpoint answers without waiting for or validating a request body.

`/healthz` only confirms that the process can accept and answer this request.
It does not check tunnel connectivity, peer reachability, proxy traffic, or
whether the process is ready to serve a particular workload. Use authenticated
`/status` or the metrics endpoint for operational diagnosis; do not treat
`/healthz` as a readiness signal.

## Configure the listener

For a probe running in the same host network namespace, bind to loopback:

```toml
[admin]
listen = "127.0.0.1:9090"
token = "replace-with-a-long-random-token"
```

The probe does not need to send the token. If the probe runs from another
container or network namespace, bind to an address reachable from that probe,
restrict access with the host/container network policy, and configure a strong
`admin.token` as required for non-loopback listeners. Keep the endpoint off the
public network. See [Admin](ADMIN.md) for listener and token details.

## Probe examples

Run a direct check from the host with:

```sh
curl --fail --silent --show-error --max-time 2 http://127.0.0.1:9090/healthz
```

For Docker Compose, the image needs `curl` installed for this example:

```yaml
services:
  espejismo:
    # image/build and command omitted
    healthcheck:
      test: ["CMD", "curl", "--fail", "--silent", "--show-error", "--max-time", "2", "http://127.0.0.1:9090/healthz"]
      interval: 30s
      timeout: 3s
      retries: 3
      start_period: 10s
```

For Kubernetes, the admin listener must be reachable from the Pod probe. Bind
to the Pod interface (commonly `0.0.0.0:9090`) and set a strong token because
the bind is non-loopback; restrict access with the cluster's network policy.
The health path itself remains unauthenticated:

```yaml
livenessProbe:
  httpGet:
    path: /healthz
    port: 9090
  periodSeconds: 30
  timeoutSeconds: 2
  failureThreshold: 3
```

Choose intervals and thresholds for the deployment's restart policy and
startup time. A failed probe indicates the process, listener, or probe network
path is unavailable; it does not identify a tunnel failure. For tunnel and
traffic state, inspect authenticated `/status` and `/metrics` instead.
