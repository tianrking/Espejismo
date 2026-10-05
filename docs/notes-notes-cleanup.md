# Notes cleanup

## Scope and rationale

The root of `docs/` contained 142 task notes, most of which record completed
work whose current guidance is already maintained in canonical documentation.
Keeping those records beside active investigations obscured the distinction
between project references and historical task logs.

Moved completed task notes into `docs/archive/notes/` and added an archive
README explaining their status. Kept three root-level notes that still track
open investigations: adaptive throughput bandwidth estimation, the C1 window
experiments, and long-transfer stability. References to the long-transfer note
from the known-issues, troubleshooting, and benchmark guides remain valid.
This change does not alter product positioning or runtime behavior.

## Expected benefit

Documentation navigation is clearer: active task notes at the docs root shrink
from 142 to 4 (the three open records plus this cleanup record), while the
completed notes remain available as history. Runtime and performance change:
0%.

## Validation

- Checked the repository for links to root-level task notes; the only external
  references were to the retained long-transfer investigation.
- `git diff --check`: passed.
- Documentation-only; cargo tests and throughput benchmarks do not apply.
