# Documentation Style

This guide defines the shared format for repository documentation. Apply it
when adding or substantially revising a page; preserve useful page-specific
structure when editing older material.

## Page structure

- Use one descriptive level-one heading as the page title.
- Start with a short paragraph that states the page's purpose and audience.
- Use level-two headings for main sections and level-three headings for
  subsections. Do not skip heading levels or use headings only for visual
  emphasis.
- Put prerequisites and important limits before steps that depend on them.
- End with validation, troubleshooting, or related links when those help the
  reader complete the task.

## Writing

- Prefer direct, concrete sentences and define project-specific terms on first
  use or link to the [glossary](../GLOSSARY.md).
- Use the same term for the same concept throughout a page. Preserve the
  project's distinction between its production TCP transport, SOCKS5 UDP
  relay, and experimental UDP underlay work.
- Describe supported behavior and known limits precisely. Do not claim
  invisibility, protocol camouflage, or guarantees that the implementation
  does not provide; see [project positioning](POSITIONING.md).
- Keep operational instructions in `docs/deployment/`, protocol contracts in
  `docs/PROTOCOL.md`, and contributor workflow in `CONTRIBUTING.md`.
- Link to a canonical guide instead of copying long instructions. Use relative
  links for repository files and descriptive link text.

## Formatting

- Use fenced code blocks with a language identifier (`bash`, `toml`, `text`,
  and similar) where applicable. Keep commands copyable and label platform-
  specific alternatives in prose.
- Use inline code for commands, paths, configuration keys, environment
  variables, and literal values.
- Use ordered lists for sequences and unordered lists for choices or facts.
- Use tables only for compact comparisons or reference data; use headings and
  lists for procedures.
- Keep lines near 80 characters where practical, without breaking URLs,
  commands, or table rows in ways that reduce readability.

## Examples and evidence

- Keep examples consistent with current CLI help, configuration defaults, and
  supported platforms. Mark placeholders clearly and never include real
  credentials, tokens, or private endpoints.
- State whether behavior is measured, tested, or an operator recommendation.
  Include method and conditions for performance figures; do not turn estimates
  into guarantees.
- For behavior or configuration changes, update the canonical user guide and
  relevant index. Follow the [contributing guide](../CONTRIBUTING.md) for
  validation and changelog expectations.

## Review checklist

- The title and opening explain the page's purpose.
- Headings follow a consistent hierarchy and match the content below them.
- Links point to existing destinations and use meaningful labels.
- Commands and configuration examples are current, safe to copy, and clearly
  identify placeholders.
- Terminology, product boundaries, and claims agree with canonical project
  references.
