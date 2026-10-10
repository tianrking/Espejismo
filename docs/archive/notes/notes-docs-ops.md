# docs-ops notes

## Scope and rationale

- Added `docs/deployment/RUNBOOK.md` for initial Linux/systemd setup, staged
  upgrade, rollback, and routine checks. The steps reflect the supplied units,
  installer behavior, diagnostics, and admin reload limitations.
- Added `docs/deployment/TROUBLESHOOTING.md` mapping common deployment symptoms
  to existing `--check-config`, `--doctor`, `--probe-server`, systemd/journald,
  admin, logging, egress, and TUN cleanup mechanisms.
- Linked both documents from the README operations index.
- Kept the existing single-config, two-binary operating model. Explicitly
  describe systemd, firewall, and route setup as operator-managed; no new
  runtime or packaging behavior is implied.

## Expected improvement

Documentation only; no runtime performance change is expected (0%). The
runbook should reduce deployment/update recovery ambiguity by documenting
preflight, backup, service restart, and post-change verification in one place.
No quantified time saving is claimed because no operator usability study was
run.

## Validation

- Compared commands and behaviors against `README.md`, deployment references,
  the installer, and the checked-in systemd units.
- Checked relative Markdown links and command/binary names manually.
- No code or configuration behavior changed, so there is no relevant Cargo
  regression suite for this documentation-only change.
- Outcome: documentation review passed; no code regressions are applicable.
