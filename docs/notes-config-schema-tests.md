# Configuration Schema Boundary Tests

## Findings and approach

`parse_config` uses Serde/TOML decoding followed by explicit semantic
validation. The schema intentionally defaults omitted top-level sections and
most fields, while entries in `remote.users` require both `name` and `psk`.
Existing tests covered a basic scalar type mismatch and several semantic
limits, but did not pin required-field failures or wrong types across nested
sections and collection fields.

Added regression tests for missing user identity/PSK, incompatible boolean,
sequence, and integer values at different schema depths, plus invalid
semantic values and the accepted `local.tun.prefix = 32` boundary. Updated the
configuration guide to clarify which omissions are defaults and which user
entry fields are mandatory. No parser behavior or runtime policy changed.

Expected improvement: guard three schema failure classes and their useful
field/type diagnostics against regressions; no performance change is expected.

## Validation

- `$HOME/.cargo/bin/cargo test -p espejismo-core config::tests:: --lib`: passed,
  40 configuration tests. New cases exercised both missing-required-field
  branches, four type/range decoding failures, three semantic rejection
  branches, and the accepted prefix upper boundary.
- `$HOME/.cargo/bin/cargo test -p espejismo-core`: passed, 282 tests across
  unit, integration, and doc targets; 1 existing loopback-bind test ignored
  under the sandbox rule. No failures.
- Correctness and diagnostics only; throughput benchmarking does not apply.
