# SIGHUP Reload Regression Tests

## Findings and approach

The remote process previously handled only Ctrl-C and SIGTERM in its main
service loop. Reload already had a single authenticated admin callback that
loads and validates a complete candidate before atomically replacing runtime
settings. Route Unix SIGHUP through that callback so signal and admin reloads
share the same transaction boundary. This is remote-only and leaves the local
process behavior unchanged.

## Change and expected effect

- Register SIGHUP once when the remote service starts. Each SIGHUP reloads the
  original config source and reapplies startup overrides through the existing
  reload action.
- A successful reload logs success; a failed reload logs the error and leaves
  the service running. Candidate construction still completes before the
  settings write lock is acquired, so readers see the old or complete new
  settings value.
- Unix SIGHUP registration failure is a startup error. Non-Unix builds retain
  their existing signal behavior.
- Expected benefit: configuration changes can be applied to the remote with a
  standard Unix service signal while preserving atomic runtime updates. No
  throughput change is expected.

## Experiment

- `cargo test -p espejismo-server` passed: 27 tests, 0 failed. The added
  `signal_reload_uses_atomic_candidate_commit` test exercises the same signal
  reload callback used by the SIGHUP branch. It verifies successful replacement
  changes the setting and a candidate validation error leaves that applied
  value intact. Existing tests also verify multi-field whole-value replacement
  and invalid-candidate preservation.
- `git diff --check` passed. Workspace-wide `cargo fmt --check` reports
  pre-existing formatting differences in unrelated files; the touched server
  source was formatted directly.
- This correctness-only change has no applicable throughput benchmark.
