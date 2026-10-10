# Security Documentation

Use this index to find Espejismo's security behavior, operational guidance,
and stated limitations. Espejismo is an authenticated encrypted tunnel; it
does not impersonate TLS, QUIC, or another protocol, and it does not claim
invisibility or protection from every form of traffic analysis. See
[project positioning](POSITIONING.md) for the product scope and
[protocol specification](PROTOCOL.md) for the wire-level contract.

## Protocol and authentication

- [Protocol specification](PROTOCOL.md) — handshake authentication, key
  derivation, encrypted frames, replay handling, and protocol limitations.
- [Authentication and key management](deployment/AUTHENTICATION.md) — PSKs,
  user credentials, handshake windows, session key updates, credential storage,
  and rotation.
- [TLS certificate boundaries](deployment/TLS-CERTIFICATES.md) — where TLS is
  and is not used, and certificate validation for HTTPS egress proxies.
- [Traffic shaping and obfuscation](deployment/OBFUSCATION.md) — padding,
  profiles, and observable traffic behavior; this is not protocol camouflage.

## Deployment and access control

- [Configuration reference](deployment/CONFIG.md) — security-related settings
  and their defaults.
- [Egress policy](deployment/EGRESS.md) — destination and port filtering,
  private-address checks, and upstream proxy behavior.
- [Admin endpoint](deployment/ADMIN.md) — optional admin listener, token
  authentication, and endpoint access.
- [Users and limits](deployment/USERS.md) — per-user PSKs, quotas, and
  bandwidth policy.
- [Configuration examples](deployment/CONFIG-EXAMPLES.md) — example values
  only; replace illustrative credentials before deployment.

## Secrets and incident handling

- [Backup and recovery](deployment/BACKUP.md) — protecting configuration
  backups and responding to suspected credential or host compromise.
- [Logging](deployment/LOGGING.md) — log content and retention considerations.
- [Security policy](../SECURITY.md) — private vulnerability reporting,
  supported-version guidance, and project security scope.
- [Troubleshooting](deployment/TROUBLESHOOTING.md) — operational diagnosis
  without sharing credentials or private configuration.

For deployment tasks, see the broader [operations index](deployment/INDEX.md).
