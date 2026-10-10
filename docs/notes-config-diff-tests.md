# Configuration Difference Tests

## Findings and approach

Configuration structs already support serialization but do not implement
structural equality. Deriving equality across the full schema would spread a
test-oriented constraint through many unrelated types. A serialized recursive
comparison provides a small, forward-compatible path diff without coupling the
schema to equality derives.

Add `changed_config_paths` to the core config module. It compares serialized
objects recursively, emits sorted dotted paths, and treats arrays as one
changed field. Results contain field names only, never configured values, so
PSKs and admin tokens are not exposed. This is a read-only helper; it does not
alter candidate validation, atomic replacement, reload behavior, or the
project's operating model.

## Change and expected effect

- Added a public helper that reports changed paths including nested fields,
  whole-list changes, additions/removals, and equal snapshots.
- Added a regression test checking deterministic ordering, multiple changed
  fields, equality, and that changed secret values never appear in results.
- Expected effect: callers and tests can identify which settings differ
  without duplicating whole-config comparison logic or disclosing values.
  Runtime reload and throughput are unchanged.

## Experiment

- `cargo test -p espejismo-core config::tests::config_diff_reports_sorted_paths_without_secret_values`: passed (1 test), covering sorted paths, nested/scalar and array differences, equality, and secret redaction by omission.
- `cargo test -p espejismo-core`: passed (173 unit tests, 5 integration/doc tests, 1 doctest).
- `cargo test -p espejismo-server reload_safety_tests`: passed (3 tests), covering whole-candidate replacement and failed-candidate preservation.
- `cargo test -p espejismo-server`: 28 passed; the existing `relay::tests::relays_tcp_through_two_socks5_hops` failed because the sandbox denied its socket operation (`PermissionDenied`).
- `rustfmt --edition 2021 --check crates/espejismo-core/src/config/mod.rs`: passed. Workspace-wide `cargo fmt --check` reports pre-existing formatting differences across unrelated files, so those files were left unchanged.
- Correctness-only change; no throughput benchmark applies.
