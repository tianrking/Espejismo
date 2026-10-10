# Issue templates

## Findings and plan

The repository had a file named `bug_report.md` containing GitHub Issue Form
`body:` metadata. GitHub expects that schema in a `.yml` or `.yaml` issue form,
so the existing bug prompts were not a functioning form. There was no dedicated
feature request form. `docs/SUPPORT.md` already provides the canonical
reporting and redaction guidance.

Replace the malformed Markdown file with a YAML bug form and add a feature
request form. The bug form gathers versions, environment, expected and actual
behavior, reproduction steps, and sanitized diagnostics. The feature form
focuses on the use case, desired outcome, and constraints. Both link back to
support guidance and avoid requesting secrets. Expected benefit: reports
arrive with more actionable context and less follow-up; this is qualitative,
and no numerical improvement is claimed. No product behavior or positioning
changes.

## Validation and outcome

- Reviewed `CONTRIBUTING.md`, `docs/SUPPORT.md`, `docs/DOCUMENTATION_STYLE.md`,
  and `docs/POSITIONING.md` to align the forms with current contributor and
  disclosure guidance.
- Checked form filenames, field IDs, required fields, and guide links against
  GitHub Issue Forms conventions and repository paths.
- Documentation-only change: Rust builds, tests, and performance benchmarks
  are not applicable; no runtime change or measured performance gain is
  claimed.
