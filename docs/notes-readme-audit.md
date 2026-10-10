# README Structure Audit

## Scope and findings

Reviewed the root README's section hierarchy, entry points, local Markdown file
links, and alignment with `docs/POSITIONING.md` and the documentation index.
The README already covers the user path from quickstart and release install
through configuration, operations, source builds, and responsible use. Its
technical claims and deliberate no-camouflage positioning match the canonical
positioning document. All existing local file links resolve in this checkout.

The landing-page navigation exposed deployment, operations, security, and
protocol references but did not provide direct links to the existing
contributor guide or license, despite the README carrying a license badge.

## Change and expected result

Added direct `Contributing` and `License` links to the README navigation. This
makes contributor workflow and license terms discoverable from the landing
page without adding duplicated instructions or changing runtime behavior.
Expected performance change is 0%; the benefit is improved navigation only.

## Validation and outcome

- Checked the README heading hierarchy and reviewed its content against
  `docs/POSITIONING.md`.
- Enumerated relative Markdown file links in `README.md`; all targets,
  including the two new links, exist locally.
- This documentation-only change does not require cargo tests or a throughput
  benchmark.
