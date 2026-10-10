# Contact and response expectations

## Findings and plan

`docs/SUPPORT.md` already directs ordinary bug reports, documentation issues,
feature requests, and setup questions to the public GitHub issue tracker. It
also states that there is no guaranteed response time. The root `SECURITY.md`
provides the separate private vulnerability reporting path and likewise makes
no response or remediation time commitment.

Make the contact model explicit in the support guide: GitHub Issues is the
public routine-support channel, no separate support email/private user-support
channel is offered, and issue review is best effort with no acknowledgement,
response, or resolution guarantee. Keep vulnerability reporting and its
handling expectations routed to `SECURITY.md`. This is documentation-only and
preserves the project's small-operations model and security boundaries. The
expected benefit is clearer expectations and fewer misdirected or urgent
support requests; no numerical improvement is claimed.

## Validation and outcome

- Checked the public issue URL against the existing README and issue-template
  links and confirmed the private reporting path in `SECURITY.md`.
- Manually verified the support guide's relative links and reviewed wording
  against `docs/DOCUMENTATION_STYLE.md` and `docs/POSITIONING.md`.
- No code or runtime behavior changed. Rust tests/builds and performance
  benchmarks are not applicable; no performance gain is claimed.
