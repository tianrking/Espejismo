# Incident postmortem template

## Findings and scope

`docs/deployment/ONCALL.md` already asks operators to record impact, timeline,
evidence, versions, verification, and follow-up. It did not provide a reusable
report structure or guidance for distinguishing confirmed causes from
hypotheses. The new template keeps post-incident review within Espejismo's
existing operator-managed deployment model; it introduces no incident service,
severity policy, or runtime behavior.

## Plan and expected result

- Add a copyable postmortem form covering summary, detection, timeline,
  technical analysis, recovery verification, follow-up ownership, and lessons.
- Keep the review blameless and evidence-based, with unknowns and confidence
  called out explicitly.
- Include secret-redaction guidance and direct operators to the existing
  on-call procedure.
- Documentation-only: no runtime, resource, or throughput change is expected
  (0%); no operational-time reduction is measured or claimed. The expected
  benefit is more consistent incident records and actionable follow-up.

## Validation

- Cross-checked recovery and verification language against
  `docs/deployment/ONCALL.md`, including the limitation of `/healthz`.
- Cross-checked scope against `docs/POSITIONING.md`; the template does not add
  operational components or change the product's deployment model.
- Documentation-only; cargo tests and throughput benchmarks do not apply.
- `git diff --check` — passed.
