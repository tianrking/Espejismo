# Docker deployment documentation

## Change and rationale

Added `docs/deployment/DOCKER.md` with the existing image's server workflow,
config and credential preparation, build/start/log/stop/update commands, client
container considerations, and pointers to related deployment docs. Updated the
Compose published port to `6690:6690`, matching the mounted example config's
`remote.listen` value; the prior `8443:8443` mapping did not reach the server
listener. Linked the guide from packaging documentation.

This is documentation and deployment-example maintenance only; it does not
change runtime behavior or Espejismo's positioning. Expected gain: users can
deploy the existing Docker image using commands consistent with its actual
listener and config instead of having to infer the setup. No performance change
is expected.

## Validation

Manually compared the documented mount path, entrypoint behavior, listener,
config filename, and Compose commands against `deployments/docker/Dockerfile`,
`deployments/docker/docker-compose.yml`, and
`configs/examples/espejismo.toml`. No tests or build were run because the change
is limited to docs and Compose port mapping; no throughput claim is applicable.
