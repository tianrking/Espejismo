# Contributing guide audit

## Findings and changes

Compared `CONTRIBUTING.md` with `.github/workflows/ci.yml`,
`docs/testing/TESTING_GUIDE.md`, `docs/testing/TEST_PLAN.md`, and
`docs/DOCUMENTATION_STYLE.md`. The contributor guide called its command list
the full quality gate but omitted `cargo check --workspace --all-targets`,
which is a CI Rust job. It also did not distinguish CI's Linux-only fuzz
manifest check from the Linux, macOS, and Windows Rust jobs or the separate
release-binary matrix. The documentation-only validation instruction referred
generically to the change review without naming concrete checks.

Added the missing workspace check and clarified which commands CI runs on
which platforms. Kept `cargo build --workspace` as a useful local build check
and described the separate release-binary job. Made the documentation-only
review actionable by naming relative-link and canonical-command checks plus
`git diff --check`.

This is a text-only workflow clarification. It does not change product
behavior or positioning. The expected benefit is fewer missed CI checks and
more consistent documentation review; this is qualitative, with no numerical
improvement claimed.

## Validation

Manually compared the revised command and platform descriptions with
`.github/workflows/ci.yml` and the testing guides. Reviewed the changed relative
links and confirmed the named workspace check is present in CI. `git diff
--check` passed. No Rust build, tests, or benchmark apply to this documentation-
only change; no performance improvement is claimed.
