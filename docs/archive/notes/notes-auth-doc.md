# Authentication Documentation

## Change and rationale

- Added `docs/deployment/AUTHENTICATION.md` describing tunnel PSK-based peer
  authentication, ephemeral X25519/HKDF session-key derivation, frame
  authentication, replay checks, handshake time windows, directional traffic
  key updates, secret storage, PSK rotation, and the separate admin token.
- Linked the guide from configuration and multi-user documentation. Existing
  protocol and operational guides remain the references for wire encoding,
  user policy, admin routes, reload, and backups.
- Documentation-only: no code, defaults, protocol behavior, or positioning
  changes. Expected runtime and performance change: 0%. The expected operator
  benefit is a single end-to-end reference that distinguishes configured
  credentials, derived session keys, and admin tokens.

## Documentation consistency check

- Compared the new explanation with `docs/PROTOCOL.md`,
  `docs/deployment/CONFIG.md`, `USERS.md`, `ADMIN.md`, and `BACKUP.md`, and
  handshake/key derivation and configuration validation in
  `crates/espejismo-core/src/crypto/mod.rs` and `config/`.
- Confirmed documented defaults: 30-second handshake slots, one previous
  slot, zero future slots; PSKs require at least 16 bytes; traffic keys update
  every configured frame interval (default 16,384 frames).
- Confirmed remote config reload updates user credentials for new handshakes;
  active sessions retain their derived session keys until disconnect.
- Result: statements match current behavior; no regression or performance
  experiment applies because runtime code is unchanged. No tests were run for
  this documentation-only change.
