# Docker Deployment

The Docker image runs `espejismo-remote` by default. The Compose example builds
that image locally, publishes the server's TCP listener, and mounts a config
file from the repository. Docker is a packaging and process-isolation option;
Espejismo keeps the same two binaries and TOML configuration model.

## Prepare the server config

Start from `configs/examples/espejismo.toml` and save a deployment copy outside
the example file, for example `deployments/docker/espejismo.toml`. Set a long,
unique secret in `shared.psk`. The sample also configures `[[remote.users]]`; if
you keep that section, set its `psk` to the same client credential. On the
server, `remote.listen` must listen on `0.0.0.0:6690` so it is reachable through
the container network. Review `[remote.egress]` for the destinations and ports
your users need.

The sample config file is mounted read-only in the container. Keep credentials
out of source control and restrict access to the host-side file.

## Build and start

From the repository root:

```bash
docker compose -f deployments/docker/docker-compose.yml build
docker compose -f deployments/docker/docker-compose.yml up -d
```

The Compose file publishes TCP port `6690` on the host. Allow that port through
the host firewall and point clients' `local.server` to `SERVER_IP_OR_NAME:6690`.
The service restarts unless stopped. Follow startup and error logs with:

```bash
docker compose -f deployments/docker/docker-compose.yml logs -f espejismo-remote
```

Validate the config before starting by running the matching release binary on
the host:

```bash
espejismo-remote --config deployments/docker/espejismo.toml --check-config
```

## Stop and update

Stop the service while retaining the image and config:

```bash
docker compose -f deployments/docker/docker-compose.yml down
```

After changing the image source or pulling a new repository revision, rebuild
and recreate the service:

```bash
docker compose -f deployments/docker/docker-compose.yml up -d --build
```

The Compose example mounts the configuration from the host, so rebuilding the
image does not replace it. Back up the host config separately.

## Client container

The same image contains `espejismo-local`; the image entrypoint can be
overridden for a client. A client config must set `local.server` to the
reachable server address and bind proxy listeners to an address reachable by
the intended host or container network. For example, publish the configured
SOCKS5 and HTTP ports with Docker Compose and set `socks5_listen` and
`http_listen` to `0.0.0.0` inside the container. Limit published proxy ports to
trusted networks or bind them to host loopback. The server Compose example does
not enable TUN; TUN requires additional host capabilities and routing setup and
is not configured here.

See [Configuration](CONFIG.md), [Troubleshooting](TROUBLESHOOTING.md), and the
[deployment quickstart](QUICKSTART.md) for the shared TOML model and binary
workflow.
