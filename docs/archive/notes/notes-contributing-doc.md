# CONTRIBUTING guide

## Findings and plan

The repository already documents its component layout in `README.md`, Rust
quality gates and smoke checks in `docs/testing/TEST_PLAN.md`, protocol and
architecture details in `docs/PROTOCOL.md` and `docs/ARCHITECTURE.md`, and the
product boundary in `docs/POSITIONING.md`. It did not have a root-level guide
that helps a new contributor find these expectations in a useful order.

Add `CONTRIBUTING.md` as a short entry point that links to those canonical
documents and summarizes the workspace setup, quality gates, focused change
workflow, documentation conventions, evidence expectations, and submission
summary. Explicitly retain the project's defining stance: authenticated
encrypted chaos, no protocol impersonation, and a small operational model.

Expected improvement: lower onboarding friction by putting the development
path and checks in one discoverable place, and reduce avoidable review cycles
caused by missing tests, docs, or benchmark evidence. This is qualitative; no
numerical improvement is claimed for a documentation-only change.

## Validation

Reviewed the new guide against the existing README, positioning statement,
architecture, protocol, changelog, and test plan. This change only adds
documentation, so no build or test run is applicable. No performance results
are applicable or claimed.
