# Backup And Recovery

Espejismo does not currently keep a database or durable tunnel state. The
important recovery material is the deployment configuration and the secrets it
contains. Back these up before changing credentials, upgrading binaries, or
replacing a host.

## What to back up

- The active TOML config on each host. It can contain `shared.psk`, per-user
  PSKs, admin tokens, and proxy credentials. Keep the backup private and
  encrypted, restrict access to the originals (for example, mode `0600`), and
  do not put real configs in source control or an image.
- Deployment files that are not recreated by installation: systemd unit
  overrides, Docker Compose changes, firewall rules, and any local scripts used
  to start the binaries. The packaged units and example config can be restored
  from the matching release.
- TUN route and DNS settings, plus any host-specific network configuration.
  These settings are in TOML, while routes themselves are operating-system
  state and should be checked and reapplied using the [TUN guide](TUN.md).
- File logs only if incident history or audit policy requires retaining them.
  Logs are not needed to restore service. Follow the [logging guide](LOGGING.md)
  for collector retention; Espejismo does not rotate file logs internally.

For Docker, back up the host-side config and Compose file. The example Compose
deployment bind-mounts the config read-only; the container image is
rebuildable. For systemd, preserve `/etc/espejismo/` and any unit overrides.
Client configs matter too: they contain credentials needed to reconnect, even
though they may be reproducible from server settings.

## Make a protected copy

On a Linux host, copy the deployment directory to a backup location accessible
only to the operator or backup service. For example:

```bash
sudo install -d -m 0700 /var/backups/espejismo
sudo install -m 0600 /etc/espejismo/espejismo.toml \
  /var/backups/espejismo/espejismo.toml
```

This creates a local copy, not an off-host backup. Include it in your encrypted
host backup or transfer it through an approved encrypted backup channel.
Avoid placing plaintext secrets in general-purpose archives or tickets. If a
config is copied to a new host, preserve restrictive ownership and permissions
there as well.

Back up before rotating a PSK or admin token, and update every peer or operator
that relies on that credential as part of the same change. Keep old backups
under the same secret-handling policy: an old PSK may still be sensitive even
after rotation.

## Restore

1. Install the intended Espejismo release and restore the deployment files.
   Stop the affected service before replacing a live config. On the supplied
   systemd layout, install the config as `root:espejismo` with mode `0640` so
   the service account can read it without making secrets world-readable:

   ```bash
   sudo systemctl stop espejismo-remote
   sudo install -o root -g espejismo -m 0640 ./espejismo.toml \
     /etc/espejismo/espejismo.toml
   ```

   Use `espejismo-local` and its service name on a client host. For Docker,
   restore the host-side config and Compose file at their mounted paths; keep
   the config readable by the account that manages the deployment.
2. Check that the config matches the role and release, then validate it while
   the service remains stopped:

   ```bash
   espejismo-remote --config /etc/espejismo/espejismo.toml --check-config
   # On a client host instead:
   espejismo-local --config /etc/espejismo/espejismo.toml --check-config
   ```

3. Restore required firewall policy and, for TUN deployments, reapply and
   verify host routes and DNS according to [TUN](TUN.md).
4. Start the service (or recreate the Docker container) and confirm its health
   and connectivity using the [runbook](RUNBOOK.md) and
   [troubleshooting guide](TROUBLESHOOTING.md).

Keep the client and remote on compatible versions; see
[version compatibility](VERSION-COMPATIBILITY.md). A config backup does not
include executable binaries, operating-system packages, firewall state, or
logs. Retain release artifacts separately if your recovery objective requires
reinstalling a specific version without network access.

## Recovery check

Periodically verify that an authorized operator can retrieve and decrypt the
backup, that the TOML parses with the intended release, and that the restore
steps still match the deployed paths. Do not test by exposing a restored
credential or service to an untrusted network.

## Disaster recovery procedure

Set recovery time and acceptable data loss targets for your own deployment.
Espejismo has no durable application data to replay; recovery time is mainly
the time needed to regain a host, retrieve protected credentials and artifacts,
and restore network policy. These targets depend on your provider and backup
schedule and are not guaranteed by the software.

### Prepare before an incident

Keep a recovery record in an operator-controlled location separate from the
hosts it describes. Record the remote and client roles, provider or console
access path, stable endpoint owner (DNS, floating IP, or load balancer), config
backup location and authorized decryption path, and the compatible release
artifacts needed for an offline restore. Include the service manager and config
paths, required firewall and egress rules, and any TUN route/DNS takeover steps.
Do not put PSKs, admin tokens, or backup decryption keys in this record.

Choose and record deployment-specific recovery time and acceptable data loss
targets, plus the operator authorized to change endpoint routing and rotate
credentials. Rehearse retrieving the backup and release artifacts and validating
a config on a clean host; record observed recovery time and any missing access
or steps. Keep recovery copies encrypted and access-controlled, and verify
that the person on call can retrieve them without access to the failed host.

### 1. Contain and assess

- For a suspected credential leak or compromised host, restrict or isolate the
  host at the provider or firewall first. Do not copy secrets from a host you
  no longer trust.
- Record the affected host, role (local or remote), last known-good release,
  config backup time, and any recent credential, firewall, route, or DNS
  changes. Keep incident logs separately if your policy requires them.
- If only the process or machine is unavailable and there is no compromise,
  preserve the existing credentials and proceed with the latest verified
  backup. Avoid rotating credentials during ordinary host replacement.

### 2. Rebuild a trusted host

Provision a clean host and restore the intended OS access controls and firewall
policy. Install the pinned Espejismo release for that host's role; retain the
matching local and remote releases as a compatible pair. Restore unit files,
overrides, Compose files, and scripts from trusted copies. Recreate service
accounts and directories using the deployment guide for [systemd](SYSTEMD.md)
or [Docker](DOCKER.md). Do not expose the listener until config validation and
firewall review are complete.

Restore the config with restrictive permissions. On a systemd deployment,
check it as the service account before startup:

```bash
sudo install -o root -g espejismo -m 0640 ./espejismo.toml \
  /etc/espejismo/espejismo.toml
sudo -u espejismo /usr/local/bin/espejismo-remote \
  --config /etc/espejismo/espejismo.toml --check-config
```

Use `espejismo-local` for a client config. For Docker, validate the host-side
mounted config with the matching release binary. Restore required firewall
rules explicitly; neither the installer nor the container image recreates
host firewall policy.

### 3. Start and verify in stages

Start the remote endpoint and check service status and logs. From an authorized
client, run `espejismo-local --config <client-config> --probe-server`; then
verify one representative SOCKS5 or HTTP request. `/healthz` only checks that
the admin HTTP handler responds, so it does not replace the client handshake
and traffic checks. See [health checks](HEALTHCHECK.md) and the
[runbook](RUNBOOK.md).

For TUN clients, treat route and DNS state as separate host state. Check the
current routes and resolver settings before enabling takeover. If a previous
client crashed during route takeover, use the saved config and
`--tun-route-cleanup` as described in the [TUN guide](TUN.md).
Do not assume restoring TOML or restarting the service reverts OS routes.

### 4. Handle suspected compromise

Rebuild on a clean host and issue new shared or per-user PSKs and admin tokens.
Update the remote and every affected client config as one coordinated change,
then validate and probe the new pair before reopening access. Disable or remove
old credentials from the remote config. Keep compromised-host backups and old
secrets protected for incident review; do not restore them into service.
Review admin exposure, firewall access, and any copied config locations.

### 5. Close recovery

Record the restored release, config backup used, credential changes, and
verification results. Confirm monitoring and backups now point at the new
host. After service is stable, remove obsolete DNS or provider routing that
would send users to the failed host. Periodically rehearse retrieval,
validation, and a clean-host restore without publishing live credentials.
