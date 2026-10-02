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

1. Install the intended Espejismo release and restore the config and deployment
   files to their original paths with restricted permissions.
2. Check that the config matches the role and release, then validate it:

   ```bash
   espejismo-remote --config /etc/espejismo/espejismo.toml --check-config
   # On a client host instead:
   espejismo-local --config /etc/espejismo/espejismo.toml --check-config
   ```

3. Restore required firewall policy and, for TUN deployments, reapply and
   verify host routes and DNS according to [TUN](TUN.md).
4. Start the service and confirm its health and connectivity using the
   [runbook](RUNBOOK.md) and [troubleshooting guide](TROUBLESHOOTING.md).

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
