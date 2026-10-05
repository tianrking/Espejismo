# Authentication And Key Management

Espejismo authenticates tunnel peers with a pre-shared key (PSK), then protects
tunnel frames with keys derived during an ephemeral X25519 handshake. The PSK
is a long-term credential; session traffic keys are derived separately and
updated while a connection is active. Authentication does not hide endpoint
addresses or traffic patterns, and is not a claim of invisibility.

## Tunnel authentication

For a single-user deployment, configure the same `shared.psk` on the local and
remote. A PSK must contain at least 16 bytes of random material; use a longer,
high-entropy value in deployments. Do not use the illustrative values in the
configuration examples. PSKs may be provided as UTF-8 text, `hex:` followed by
hexadecimal bytes, or `base64:` followed by standard Base64 bytes. The prefixes
are supported for configured and CLI PSKs.

When `remote.users` contains entries, the remote authenticates against each
entry's PSK and associates the session with that user. Each local client must
hold only its assigned PSK as `shared.psk`. If the list is empty, the remote
uses `shared.psk` (or the CLI `--psk`) as the single `default` user. Per-user
quotas and bandwidth limits are policy applied after user identification;
they are not separate authentication factors. See [Users](USERS.md) for the
per-user configuration.

The handshake authenticates the client hello with an HMAC derived from the PSK.
The peers exchange ephemeral X25519 public keys and derive directional
XChaCha20-Poly1305 traffic keys using HKDF, the PSK, and handshake material.
The server reply is authenticated before the client accepts the session.
Subsequent frames are authenticated and encrypted; a failed frame
authentication closes the physical connection. The remote also tracks recent
authenticated first-packet digests and ephemeral public keys to reject replay.
See [Protocol](../PROTOCOL.md) for the wire-level sequence and derivations.

### Handshake time windows

`shared.handshake_window.enabled` derives the handshake authentication key
from the PSK and a time slot. Both peers must use compatible window settings
and reasonably synchronized clocks. The remote accepts the current slot plus
the configured previous and future slots; defaults are a 30-second step, one
previous slot, and no future slots. This tolerance applies to new handshakes,
not to already established sessions. It does not replace the replay cache.

## Session key updates

Each direction updates its traffic secret independently after
`shared.key_update_frames` transmitted frames. The authenticated key-update
frame lets the receiver derive the next traffic secret and header-mask key.
This limits how long one frame key is used on a long-lived connection. It does
not rotate or replace the configured PSK. Both peers must use compatible
protocol settings; consult [version compatibility](VERSION-COMPATIBILITY.md)
when upgrading.

## Protect credentials at rest

PSKs, admin tokens, and upstream proxy credentials are secrets. Keep real
configuration files out of source control, container images, tickets, and
general-purpose logs. Restrict file ownership and permissions (for example,
mode `0600` on Unix), limit who can read backups, and encrypt off-host backups.
CLI arguments can be visible to local process inspection or shell history, so
prefer a protected configuration file for persistent service credentials.
Before sharing diagnostic output, remove credentials and private config data.

## Rotate a tunnel PSK

There is no overlapping old/new PSK slot for ordinary PSK rotation. A
credential change must be coordinated between the remote and every client
using that identity. For multi-user deployments, rotate only the selected
`remote.users` entry and its clients; do not distribute another user's key.

1. Make a protected backup of the current configuration.
2. Generate a new random PSK of at least 16 bytes (prefer a longer secret).
3. Update the remote's `shared.psk` or selected `remote.users[].psk`, and the
   matching client's `shared.psk`.
4. Validate each config with the corresponding binary's `--check-config`, then
   apply the update using the deployment's restart or authenticated reload
procedure.
5. Confirm that a client using the new key reconnects successfully. Remove
   old copies from active configs and handle backups under the same secret
   policy; backups containing a retired PSK remain sensitive.

Existing connections may continue with their established session keys until
they disconnect; a PSK change controls authentication of new handshakes. Plan
the change with that behavior in mind. See [Admin API](ADMIN.md#configuration-reload)
and [Backup and Recovery](BACKUP.md) for related operational details.

## Admin endpoint credentials

The admin token is separate from the tunnel PSK. Keep the admin listener on
loopback where possible. A non-loopback listener requires a non-empty
`admin.token`; all routes except the liveness probe require authentication.
Use a high-entropy token, send it only over a trusted path, and rotate it by
updating the config and applying/restarting the service. See [Admin API](ADMIN.md)
for routes and request headers.

## Local proxy credentials

`local.auth` is optional and applies only to the client's SOCKS5 and HTTP
proxy listeners; it does not authenticate the tunnel peer. These listeners
default to loopback and have no proxy authentication unless `local.auth` is
configured. Keep them on loopback, or restrict access with host firewall and
network policy before binding them to a reachable address. SOCKS5
username/password negotiation and HTTP `Proxy-Authorization: Basic` do not
encrypt credentials on the local proxy connection, so use a trusted local
path. This is separate from the authenticated, encrypted client-to-remote
tunnel.
