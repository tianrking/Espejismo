# Acknowledgments page notes

## Scope and rationale

Added `docs/ACKNOWLEDGMENTS.md` to provide a durable place to recognize code,
testing, issue-reporting, documentation, and feedback contributions. The page
links to GitHub's maintained contributors graph instead of hard-coding names,
which avoids an incomplete or stale roster. It also acknowledges upstream
open-source work and points to the manifests that identify dependencies,
without implying endorsement or affiliation.

Linked the page from the README navigation and development documentation
index. This is documentation-only and does not change Espejismo's product
positioning or runtime behavior. Expected benefit is improved visibility of
contributors and a clear recognition entry point; this is qualitative, with
no numerical improvement claimed.

## Validation and outcome

- Checked the new relative links against the repository paths and verified the
  GitHub contributors URL follows the repository's existing owner/name.
- Compared wording with `docs/DOCUMENTATION_STYLE.md` and
  `docs/POSITIONING.md`.
- `git diff --check`: passed.
- No code changed, so Rust builds, tests, and performance benchmarks do not
  apply; no performance improvement is claimed.
