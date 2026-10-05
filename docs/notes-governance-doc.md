# Governance documentation notes

## Scope and rationale

Reviewed `CONTRIBUTING.md`, `docs/POSITIONING.md`,
`docs/DOCUMENTATION_STYLE.md`, and the development documentation index. The
contributing guide already described routine changes, review expectations, and
validation, but it did not say which decisions need an explicit rationale or
how to handle proposals that challenge product or protocol boundaries.

Added a decision process to `CONTRIBUTING.md`. Routine focused changes need a
clear problem, proposal, and validation in the pull request. Decisions with
architectural, compatibility, security, behavior, or operational impact call
for context, alternatives, expected effects, and evidence before implementation.
The process also directs contributors to pause and discuss conflicts with the
positioning or protocol specification, and to preserve lasting rationale in
canonical development documentation.

This is documentation-only and preserves the project's small operating model,
non-camouflage positioning, and existing review authority. Expected benefit is
clearer proposals and review outcomes, with less rationale lost in transient
discussion; no runtime or performance improvement is expected or claimed.

## Validation

- Compared the new guidance with the existing contributing, positioning,
  documentation-style, protocol, and development-index guidance.
- Checked the added relative links against the repository paths; both targets
  exist.
- `git diff --check`: passed.

No Rust build or tests are applicable because no code or behavior changed.
