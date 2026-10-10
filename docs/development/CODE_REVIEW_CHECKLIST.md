# Code review checklist

Use this checklist to review a focused change against its stated problem,
Espejismo's architecture, and the evidence provided by its author. Skip items
that do not apply and note why when that helps future readers. The checklist
complements the detailed workflow in the [contributing guide](../../CONTRIBUTING.md).

## Scope and design

- [ ] The diff solves the stated problem and does not include unrelated work.
- [ ] Ownership stays in the appropriate crate and module; new public surface
  area is intentional and documented.
- [ ] The design preserves Espejismo's small operating model and its refusal to
  impersonate TLS, HTTP, QUIC, or another protocol.
- [ ] Any decision affecting architecture, protocol compatibility, security
  boundaries, supported behavior, or operational complexity has an explicit
  rationale and review outcome.

## Behavior and failure handling

- [ ] Normal behavior and relevant failure cases are handled explicitly;
  errors are propagated or reported with useful context.
- [ ] Network input, configuration, and runtime state are validated before
  use, allocation, or state changes.
- [ ] Async work has clear ownership, cancellation and shutdown behavior, and
  bounded queues, buffers, and peer-controlled resource use.
- [ ] State transitions, retries, timeouts, and cleanup remain correct on
  partial failure and connection shutdown.

## Security and protocol

- [ ] Authentication, encryption, replay defense, and trust boundaries remain
  explicit; unauthenticated probes do not gain useful detail.
- [ ] Secrets and private payloads are not exposed in logs, errors, or metrics.
- [ ] Wire-format, handshake, or compatibility changes match the
  [protocol specification](../PROTOCOL.md) and identify affected peers and
  rollout requirements.
- [ ] Security and visibility claims match the [project positioning](../POSITIONING.md);
  encrypted chaos is not described as invisibility or camouflage.

## Tests and evidence

- [ ] Behavior changes have focused regression coverage, including relevant
  malformed-input and boundary cases.
- [ ] The author reports the relevant checks actually run and their results;
  skipped checks and platform limits are stated accurately.
- [ ] Performance claims include reproducible conditions and comparable
  measurements; security claims have evidence appropriate to the claim.
- [ ] Test failures are understood and are not dismissed as unrelated without
  a concrete reason.

## Documentation and compatibility

- [ ] User-facing behavior, configuration, CLI, deployment, and compatibility
  documentation are updated where needed and agree with the implementation.
- [ ] Examples and commands are current, safe to copy, and use clearly marked
  placeholders rather than real credentials or endpoints.
- [ ] Relative links resolve, terminology is consistent, and claims describe
  supported behavior and known limits precisely.
- [ ] Changelog and migration guidance reflect user-visible, operational,
  security, or compatibility effects when applicable.

## Final pass

- [ ] The final diff contains no generated output, secrets, local build
  artifacts, or unrelated formatting changes.
- [ ] Review comments distinguish required fixes from suggestions and are
  specific enough to act on.
- [ ] Requested changes are addressed, affected checks are rerun, and the PR
  description still matches the final diff and validation results.

For change workflow, validation commands, and review discussion expectations,
see the [contributing guide](../../CONTRIBUTING.md). For the canonical test
scope, use the [test plan](../testing/TEST_PLAN.md) and
[testing guide](../testing/TESTING_GUIDE.md).
