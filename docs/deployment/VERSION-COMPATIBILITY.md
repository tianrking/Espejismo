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

## Release support and end of life

Only the latest stable Espejismo release is supported. A stable release reaches
end of life (EOL) when a newer stable release is published. EOL releases no
longer receive routine maintenance or security fixes; users should move to the
latest stable release. The project does not promise a minimum support duration,
long-term-support branch, or backports to EOL releases. Development builds and
pre-releases are not supported production releases.

Check the project's release page for the current stable release and its notes.
Those notes define any release-specific migration steps and compatibility
exceptions. The support policy does not imply that a newer release can
interoperate with an older peer; use the compatibility guidance below and
coordinate upgrades when a pairing is not explicitly documented.

## Compatibility matrix

The matrix records what the maintained documentation establishes. “Not
verified” means there is no release-pair guarantee; plan a coordinated upgrade
unless the exact pair is covered by that release's notes.

| Client release | Server release | Wire protocol evidence | Deployment guidance |
| --- | --- | --- | --- |
| `v0.1.5` | `v0.1.5` | Protocol specification for the `v0.1.x` line through `v0.1.5` defines wire version `1`; both binaries use the workspace release version. | Same-release pairing is the documented baseline. Keep compatible configuration and authentication settings on both peers. |
| `v0.1.0`–`v0.1.4` | `v0.1.0`–`v0.1.4` | Per-release wire-version assignments and cross-release test results are not recorded in the maintained compatibility references. | No pairwise compatibility guarantee is documented. Do not infer compatibility from matching release numbers. |
| Any `v0.1.0`–`v0.1.4` | `v0.1.5` (or reverse) | The current protocol document does not establish the earlier release's wire version or a compatibility window. | Treat as unverified; upgrade both sides together unless the applicable release notes establish this exact pair. |
| Any future release | Any earlier release | Depends on the target release's protocol, config, and CLI changes. | Use a cross-version pairing only when the target release notes explicitly guarantee it; otherwise coordinate the upgrade. |

This is an evidence matrix, not a claim that unverified pairs fail. Update it
when release artifacts or release notes establish a protocol version or a
tested client/server pair. Wire compatibility is only one part of an upgrade:
configuration and CLI compatibility remain separate, as described below.

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
