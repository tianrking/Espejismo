# Backup and recovery documentation

## Findings and scope

`docs/deployment/BACKUP.md` already described backup contents, disaster
recovery, credential compromise, and TUN state. The restore overview did not
make the service stop/replacement order explicit or show the ownership and mode
needed by the supplied systemd service account. The systemd guide establishes
`root:espejismo` ownership and mode `0640`; restoring with a generic restrictive
mode can otherwise leave the service unable to read its config.

This round clarifies the restore sequence for systemd and Docker without
changing the deployment model or runtime behavior.

## Plan and expected result

- Stop the service before replacing a live configuration.
- Show the systemd config install command using the documented service group
  and permissions; distinguish Docker's host-mounted config restore.
- Keep validation ahead of restarting/recreating the service.

Expected benefit: fewer failed recoveries caused by unreadable config files or
replacing files while the process is running. No performance change is
expected or claimed.

## Validation

- Cross-checked the systemd owner, group, and mode against
  `docs/deployment/SYSTEMD.md` and the supplied service unit.
- Cross-checked the Docker host-side mount behavior against
  `docs/deployment/DOCKER.md`.
- Reviewed the restore flow and Markdown code blocks. This is documentation
  only; cargo tests and throughput benchmarks do not apply.
