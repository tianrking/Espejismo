# Contribution guide expansion

## Findings and plan

`CONTRIBUTING.md` already described the Rust quality gates, focused changes,
documentation conventions, and the information contributors should provide
when submitting work. It did not explain the PR description in a reusable
structure or set expectations for reviewers and revision rounds.

Expand the submission section with guidance for focused PRs, motivation and
impact, compatibility and deployment effects, and honest validation reporting.
Add reviewer checks for scope, architecture, regression coverage, security
boundaries, and documentation accuracy. Describe actionable review feedback,
author responses, rerunning affected checks, and the final pre-merge review.

This is a documentation-only change. The expected benefit is fewer review
rounds caused by missing context, validation results, or unclear ownership of
requested revisions. That benefit is qualitative; no numerical improvement
is claimed. The additions preserve the project's small operational model and
its no-impersonation positioning.

## Validation

Manually compared the new guidance with the existing quality gates in
`CONTRIBUTING.md`, the documentation conventions in
`docs/DOCUMENTATION_STYLE.md`, and product boundaries in
`docs/POSITIONING.md`. The change only affects contributor documentation, so
no Rust build, test, or performance benchmark applies. No performance change
or measured improvement is claimed.
