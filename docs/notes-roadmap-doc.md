# Roadmap documentation

## Scope and rationale

Added `docs/ROADMAP.md` as a public overview because the repository had no
roadmap page. Its near-term focus comes from the existing long-transfer
investigation and ongoing reliability/operations work. Follow-on areas are
limited to gaps already recorded in `docs/development/STATUS.md`; they are
explicitly described as unscheduled directions rather than commitments.

Added a README entry so users can find it. The page links back to implementation
status, known issues, protocol documentation, and project positioning. It
preserves the project's non-camouflage approach and small operational model.

## Expected benefit

Users and contributors get one place to understand current priorities and
known future gaps without mistaking exploratory items for release promises.
Runtime, correctness, and performance change: 0% (documentation only).

## Validation

- Reviewed roadmap claims against `docs/POSITIONING.md`,
  `docs/development/STATUS.md`, and `docs/KNOWN_ISSUES.md`.
- Confirmed referenced local documentation paths exist.
- `git diff --check`: passed.
- Checked relative Markdown links in the README and roadmap; all targets exist.
- Cargo tests and throughput benchmarks do not apply to this documentation-only
  change.
