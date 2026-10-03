# Client and Server Version Compatibility

Espejismo has two version numbers to keep distinct:

- **Binary release version** (for example, `0.1.5`) identifies the released
  client and server programs. The workspace currently assigns both binaries
  the same release version.
- **Wire protocol version** is carried in the authenticated handshake. The
  current protocol specification defines version `1`.

The binary release number is not a compatibility negotiation mechanism. The
handshake currently requires an exact protocol-version match: a peer rejects
an unsupported client or server protocol version. There is no automatic
fallback to an older protocol. Consequently, equal `0.1.x` numbers do not
prove that arbitrary builds interoperate, and unequal release numbers do not
alone prove that they cannot interoperate.

Configuration compatibility is a separate concern. TOML has no schema-version
field, and the current parser rejects unknown keys, including keys that a
target release has removed or renamed. A config that parses on the old binary
may therefore fail validation on the new one. Read release-specific config
migration notes, edit a copy of the saved config, and validate it with both
target binaries' `--check-config` commands before replacing the live files.
Preserve secrets and intentional egress restrictions when comparing with a
new example config; do not replace a working config wholesale just to pick up
new defaults. See [Configuration](CONFIG.md) for parser behavior.

## Upgrade policy

Use the release notes and protocol specification for the target release as the
compatibility authority. Unless they explicitly document support for the
existing peer version, treat a client/server release pair as requiring a
coordinated upgrade. Do not assume a rolling upgrade is safe.

When a release explicitly guarantees backward compatibility with the running
peer, upgrade the **server first**, verify it, then upgrade clients in a
controlled batch. This order keeps the shared endpoint updated before client
rollout and makes a client-side rollback straightforward. Check the exact
release notes for any exception or required order.

When compatibility is not documented, schedule a maintenance window and
upgrade server and clients as one change. A client connecting while the server
has an incompatible protocol can fail the handshake; the binaries do not
negotiate a common version. Existing tunnels may also be interrupted by
service restarts.

## Upgrade checklist

1. Pin the target release and read its release notes, protocol changes, and
   configuration migration notes.
2. Record both installed binary versions and back up binaries and protected
   configuration. Keep PSKs and shared settings paired.
3. Run each target binary's `--check-config` against the saved configuration.
4. Follow the documented compatibility order. If no cross-version guarantee
   exists, replace both sides in the same maintenance window.
5. Confirm service health, run the client's `--probe-server`, and verify a
   representative proxy request before completing rollout.
6. If validation fails, restore the previous compatible binary pair and its
   matching configuration. Do not leave a mixed pair unless that exact pair
   is documented as compatible.

See the [upgrade and rollback runbook](RUNBOOK.md) for service commands and
the [protocol specification](../PROTOCOL.md) for the current wire contract.
