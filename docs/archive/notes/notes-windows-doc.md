# Windows deployment documentation

## Change and rationale

- Added `docs/deployment/WINDOWS.md` with native PowerShell install/configure,
  client and server startup, firewall scope, credential file handling, and
  optional elevated TUN operation and crash cleanup.
- Linked the guide from `docs/deployment/QUICKSTART.md` and included it in the
  full and server-only release package documentation lists. This keeps the link
  available to operators using either the repository or extracted archives.
- Checked the descriptions against `scripts/install.ps1`, the CLI examples,
  `docs/deployment/TUN.md`, and the Windows packaging steps. The installer
  extracts files only; it does not register a service or alter firewall rules.
- Preserves the explicit two-binary, one-TOML operating model. There is no
  runtime or performance change; expected throughput improvement is 0%.
  The expected benefit is fewer Windows-specific setup and recovery mistakes;
  no operator time reduction is claimed without usability measurements.

## Validation

- Manually checked command paths against the install directory and release
  package layout, TUN privilege/DNS limitations against `TUN.md`, and all
  relative links in the new guide and Quickstart.
- Confirmed the release workflow copies `WINDOWS.md` into both full and
  server-only package docs on Unix and Windows runners.
- Documentation and packaging-list change only. No Cargo tests or benchmarks
  were run; there is no runtime behavior or performance claim to verify.
- Outcome: documentation review passed; no code regressions are applicable.
