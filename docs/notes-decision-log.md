# Decision log

## ADR template

- **Change:** Added [`ADR_TEMPLATE.md`](ADR_TEMPLATE.md) for recording
  consequential architecture and protocol decisions.
- **Reason:** The repository has meeting and incident templates, but no
  consistent record for decisions whose context and trade-offs should remain
  discoverable after implementation.
- **Approach:** Capture status, date, decision makers, context, options,
  consequences, validation, and review triggers. Keep the template adaptable;
  use it when a decision affects component boundaries, protocol behavior,
  security properties, or long-term maintenance, rather than requiring an ADR
  for routine implementation choices.
- **Expected benefit:** Future maintainers can find the rationale and
  conditions for revisiting a significant decision in one place. This is a
  documentation and discoverability improvement; no runtime or performance
  change is expected or claimed.
- **Validation:** Manually checked the template structure against the existing
  meeting-notes and postmortem templates and confirmed its guidance remains
  consistent with [`POSITIONING.md`](POSITIONING.md). No code or behavior
  changed; tests are not applicable.
