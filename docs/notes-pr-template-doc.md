# PR template

## Findings and plan

`CONTRIBUTING.md` already asks pull requests to explain the problem, effect,
approach, compatibility impact, and validation. It also calls for platform
limits and comparable benchmark conditions when relevant. The repository had
GitHub issue forms but no pull request template, so those expectations were
not presented to authors while opening a PR.

Add `.github/pull_request_template.md` with prompts for the summary, approach,
compatibility and operational effects, checks and platform coverage, and
performance evidence. Include a short checklist for scope, project positioning,
docs, and generated files or secrets. This keeps the project boundaries visible:
the core tunnel does not impersonate another protocol, and security or
operational effects should be stated explicitly. Expected benefit is more
consistent, reviewable PR descriptions and fewer missing validation details;
this is qualitative, with no numerical improvement claimed. No product or
runtime behavior changes.

## Validation and outcome

- Reviewed `CONTRIBUTING.md`, `docs/POSITIONING.md`,
  `docs/DOCUMENTATION_STYLE.md`, and the existing GitHub issue forms to align
  prompts with repository guidance and terminology.
- Confirmed the PR template is at GitHub's recognized repository path,
  `.github/pull_request_template.md`.
- Documentation-only change: runtime builds, tests, and benchmarks are not
  applicable; no performance gain is claimed. Checked changed local references
  and whitespace with `git diff --check`.
