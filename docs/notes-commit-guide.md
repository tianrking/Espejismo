# Commit Guide Documentation Notes

## Change and rationale

Added `docs/COMMIT_GUIDE.md` to define concise imperative English subjects,
optional explanatory bodies, evidence-based validation wording, and message
content that describes the change itself. Linked it from the contributor
submission guidance so authors can find it while preparing commits.

The repository history already uses short imperative subjects and bodies that
explain motivation and validation where useful. Documenting that pattern gives
contributors one reference and reduces inconsistent or vague history entries.
It does not change project behavior or positioning.

## Expected benefit

This is a process and documentation improvement; no runtime performance gain
is expected. Clearer commit subjects and bodies should make history easier to
scan and change intent easier to recover during review or maintenance. That
benefit is qualitative and is not presented as a measured percentage.

## Validation

- Reviewed recent commit subjects and bodies for consistency with the guide.
- Checked the new relative link from `CONTRIBUTING.md` and the guide's link to
  `DOCUMENTATION_STYLE.md` against the repository paths.
- Ran `git diff --check` successfully.
- Build, tests, and benchmarks are not applicable to this documentation-only
  change; no runtime or performance claim is made.
