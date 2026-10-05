# Testing Guide Notes

## Scope and rationale

- Added `docs/testing/TESTING_GUIDE.md` as a contributor-facing route through
  the existing checks: package-scoped tests, the CI-matching workspace gate,
  bounded fuzz runs, manual proxy smoke checks, and benchmark references.
- Linked the guide from `docs/development/INDEX.md` so contributors can find it
  from the development documentation entry point.
- Existing `TEST_PLAN.md`, `BENCHMARKS.md`, `CONTRIBUTING.md`, fuzz README, and
  CI workflow already define commands and coverage. The new page organizes
  them by task instead of duplicating their detailed coverage and measurement
  instructions.
- Expected improvement: easier selection of a relevant validation path and
  fewer mistaken claims that local or single-platform checks cover all CI
  targets. This is a documentation usability benefit; no runtime or test
  coverage change is claimed.

## Validation

- Checked command names and CI platform statements against
  `.github/workflows/ci.yml`, `CONTRIBUTING.md`, `docs/testing/TEST_PLAN.md`,
  `docs/testing/BENCHMARKS.md`, and `fuzz/README.md`.
- Checked the new relative links against repository paths.
- Ran `$HOME/.cargo/bin/cargo test --workspace --all-targets`. The client (31),
  core (130), core documentation example (1), server (19), and tokio-yamux unit
  tests (24) passed. The final `tokio-yamux` integration test
  `one_way_bulk_transfer_exceeding_window` failed before exercising the
  transfer: `crates/tokio-yamux/tests/window_update_deadlock.rs:31` received
  `Operation not permitted` while creating its socket in this sandbox. Thus
  the full workspace test command did not pass; rerun it in an environment
  that permits the test's local socket operation.
- No performance experiment applies to this documentation-only change.
