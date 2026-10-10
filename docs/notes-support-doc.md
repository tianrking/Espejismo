# Support documentation

## Plan

- Add `docs/SUPPORT.md` as the canonical place for the public issue channel,
  reporting template, and redaction guidance.
- Link it from the README, operations index, and contribution guide so both
  users and contributors can find it without duplicating the template.
- Keep vulnerability disclosure routed to the existing private process in
  `SECURITY.md`; this preserves the project's security boundaries.

The template asks for versions, operating systems, redacted configuration,
reproduction steps, expected/actual behavior, and sanitized diagnostics. This
should reduce follow-up needed to reproduce routine reports; no quantitative
improvement is claimed for this documentation-only change.

## Validation and outcome

- Confirmed the repository URL from the configured `origin` remote and used its
  existing GitHub Issues path.
- Confirmed private vulnerability reporting instructions in the root
  `SECURITY.md`; the support guide links there rather than inviting public
  disclosure.
- Manually checked all new relative links against repository paths and reviewed
  the diff for consistency with `docs/DOCUMENTATION_STYLE.md` and
  `docs/POSITIONING.md`.
- No code or runtime behavior changed; Rust tests/builds and performance
  benchmarks are not applicable. Expected project behavior and positioning are
  unchanged.
