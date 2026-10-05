# Translation guide

## Findings and plan

The repository has an English `README.md` and a Spanish `README_ES.md`, but no
shared workflow for adding translations or keeping them synchronized. The
existing documentation style guide covers prose and links generally, while
`CONTRIBUTING.md` does not explain translation-specific review.

Added `docs/TRANSLATION_GUIDE.md` to define English as the reference, preserve
copyable technical material, explain source/translation linking and staleness,
and require translation review against canonical behavior and project
positioning. Linked it from `CONTRIBUTING.md` and the development documentation
index. This is documentation-only; the expected improvement is qualitative:
fewer translation mismatches and clearer contributor review. No numerical
performance or runtime improvement is claimed.

## Validation

Manually reviewed the guide against `docs/DOCUMENTATION_STYLE.md`,
`docs/POSITIONING.md`, `CONTRIBUTING.md`, and the existing English/Spanish
README pairing. Checked the added relative links and terminology by inspection.
No build or tests apply to this documentation-only change. `git diff --check`
is recorded after editing; no performance experiment applies.
