# Changelog

## v0.0.2

`v0.0.2` turns Espejismo from a minimal encrypted proxy prototype into a more
operable native tunnel release.

### Added

- Multi-user remote authentication with independent per-user PSKs.
- Per-user rolling byte quotas and aggregate bandwidth limits.
- Server-side SOCKS5 chained egress for TCP and UDP. TCP uses CONNECT; UDP uses
  SOCKS5 UDP ASSOCIATE.
- Remote admin runtime management:
  - `POST /reload` re-reads the startup config source.
  - `POST /apply` accepts a TOML config body and applies runtime settings.
- Config import/export CLI:
  - `--print-config-base64`
  - `--decode-config-base64`
  - existing `--config-base64`
- Client profile export/import through `espejismo://import/...`.
- Release update checks for both binaries with `--check-update` and
  configurable `--update-url`.
- UDP underlay core primitives: packet codec, session id, sequence numbers,
  cumulative ACK, retransmission scheduling, and congestion window logic.
- Deployment docs for users, egress, admin, update checks, packaging, logging,
  profiles, and quick start.

### Changed

- Release packages now include the expanded deployment documentation set.
- Smoke tests now verify config conversion, update checks, runtime `/apply`,
  SOCKS5 TCP, SOCKS5 UDP, HTTP proxy, HTTP CONNECT, admin, metrics, profiles,
  and packaging.

### Known Limits

- The stable physical tunnel still uses TCP/yamux. UDP underlay socket
  integration is prepared in core primitives but is not yet the default runtime
  transport.
- Runtime apply affects new physical tunnels and newly opened logical streams.
  Process-owned resources such as listener addresses and log file handles still
  require restart.

## v0.0.1

- Initial native Rust workspace.
- Encrypted TCP physical tunnel with yamux logical streams.
- SOCKS5 and HTTP local proxy ingress.
- X25519 key exchange, XChaCha20-Poly1305 frames, replay cache, client puzzle,
  adaptive padding, stealth profile, admin metrics, packaging, and smoke tests.
