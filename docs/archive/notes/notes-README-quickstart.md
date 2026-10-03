# README Quickstart Review

## Findings and change

- The README described installation and a combined TOML example, but did not
  provide an end-to-end path from a fresh install to a tested local proxy.
- The release installer defaults to `~/.espejismo` on Linux/macOS and the
  release package includes `configs/espejismo.toml`; the documented copy path
  and binary paths match those artifacts.
- Added a Linux/macOS quickstart in `README.md`: install on both hosts, set the
  same PSK in `[shared].psk` and the server's `[[remote.users]].psk`, configure
  the client's remote address, open TCP 6690 on the server, run the config
  check and handshake probe, then test the SOCKS5 listener.
- The full shared TOML example is intentionally reused on both hosts; the
  client needs `[local].server` changed and the server keeps `[remote].listen`
  on `0.0.0.0:6690`. This preserves the project's one-file configuration model.

## Rationale and expected result

The added sequence makes host-specific edits and the network prerequisite
explicit, and gives users a concrete success check. Expected result: a new
Linux/macOS user with a prepared reachable server can follow the README to a
working SOCKS5 proxy in about five minutes, excluding package download time and
any firewall administration delay. This is a usability target, not a measured
runtime benchmark.

## Review / validation

- Cross-checked install paths and platform defaults against `scripts/install.sh`
  and `scripts/install.ps1`.
- Cross-checked package contents against `.github/workflows/release.yml` and
  `docs/deployment/PACKAGING.md`.
- Cross-checked config keys, ports, CLI diagnostics, and proxy listener defaults
  against `configs/examples/espejismo.toml`, `docs/deployment/QUICKSTART.md`,
  `docs/deployment/CLI.md`, and server authentication handling in
  `crates/espejismo-server/src/main.rs`.
- Documentation-only change; no source behavior changed, so no benchmark or
  Cargo test is applicable. No timed first-run usability study was performed.
