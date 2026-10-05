# Development documentation

Use this index to find the project rules, technical references, and checks
needed to make a change to Espejismo.

## Start here

- [Contributor onboarding](../ONBOARDING.md) — a short path from checkout to
  a focused change and review.
- [Contributing guide](../../CONTRIBUTING.md) — workspace setup, change workflow,
  quality gates, and documentation conventions.
- [Code review checklist](CODE_REVIEW_CHECKLIST.md) — focused review prompts
  for behavior, security, tests, documentation, and final diff.
- [Acknowledgments](../ACKNOWLEDGMENTS.md) — contributor recognition and
  upstream open-source acknowledgments.
- [Documentation style](../DOCUMENTATION_STYLE.md) — shared page structure,
  writing, formatting, and review conventions.
- [Translation contributions](../TRANSLATION_GUIDE.md) — workflow for adding
  and maintaining accurate translations.
- [Project positioning](../POSITIONING.md) — product boundaries and principles
  that changes must preserve.
- [Architecture](../ARCHITECTURE.md) — crates, runtime components, and their
  responsibilities.
- [Code tour](../CODE-TOUR.md) — workspace map and source paths for the client,
  server, and shared core.

## Technical references

- [Protocol specification](../PROTOCOL.md) — wire format, negotiation, and
  compatibility behavior.
- [Security overview](../SECURITY.md) — security properties and relevant
  guidance.
- [Research design principles](../research/DESIGN_PRINCIPLES.md) — design
  constraints and rationale.
- [Implementation status](STATUS.md) — implemented capabilities, known gaps,
  and architecture direction.

## Validation and evidence

- [Testing guide](../testing/TESTING_GUIDE.md) — choose fast, full, manual,
  fuzz, and performance checks for a change.
- [Test plan](../testing/TEST_PLAN.md) — automated coverage and manual smoke
  checks.
- [Performance index](../testing/PERFORMANCE_INDEX.md) — benchmark method,
  tuning guidance, and recorded results.
- [Benchmark methodology](../testing/BENCHMARKS.md) — how to produce comparable
  throughput measurements.
- [CPU and memory profiling](PROFILING.md) — collect Linux CPU samples and
  memory observations for representative workloads.

## Project history and release work

- [Project history](HISTORY.md) — major stages and design direction across
  releases.
- [Changelog maintenance](CHANGELOG.md) — user-facing entries and release
  preparation.
- [Release notes template](../release/RELEASE_NOTES_TEMPLATE.md) — structure
  versioned release notes consistently.
- [Versioning and release branches](VERSIONING.md) — package versions, release
  tags, and the branch policy.
- [API and configuration deprecation](DEPRECATION.md) — announce and remove
  public interfaces with migration guidance.
- [API stability](API-STABILITY.md) — compatibility boundaries for Rust,
  configuration, CLI, admin, and wire interfaces.
- [Release checklist](../release/RELEASE_CHECKLIST.md) — release readiness
  checks.

Operational instructions belong in the [deployment index](../deployment/INDEX.md).
