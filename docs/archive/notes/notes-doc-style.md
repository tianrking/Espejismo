# Documentation style guide

## Findings and plan

- Reviewed the contributor guide, development and deployment indexes, project
  positioning, and representative setup and glossary pages. The repository has
  206 Markdown files under `docs/`; those pages have differing heading labels,
  examples, and levels of detail, with conventions previously stated only
  briefly in `CONTRIBUTING.md`.
- A mechanical rewrite of all 206 files risks changing technical meaning and
  creating broad review noise. Establish one explicit style contract and make
  it discoverable from the contributor workflow and development index. Apply
  it as pages are edited, keeping canonical behavior documentation intact.
- The guide covers hierarchy, plain language, terminology, placement of
  instructions, links, code examples, evidence, and a review checklist. It
  explicitly retains the product boundaries in `POSITIONING.md`.

## Change and expected benefit

Added `docs/DOCUMENTATION_STYLE.md` and linked it from `CONTRIBUTING.md` and
`docs/development/INDEX.md`. This gives contributors one practical reference
and reduces future style drift across user and developer documentation. The
expected benefit is more consistent navigation and examples; there is no
runtime or protocol performance change (0%).

## Validation and outcome

- Reviewed the new rules against the current positioning and glossary, and
  checked the two links to the style guide and its glossary reference.
- This is a documentation-only change. Cargo tests and throughput benchmarks
  do not apply; executable files and runtime behavior are unchanged.
- Outcome: shared conventions are now documented and discoverable. The full
  legacy corpus has not been mechanically reformatted; the guide is the
  standard for new and substantially revised pages, to avoid altering
  technical meaning through blanket edits.
