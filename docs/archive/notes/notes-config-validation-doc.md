# Config validation documentation

## Analysis and changes

`docs/deployment/CONFIG.md` described validation commands and a few individual
limits, but did not explain the distinction between TOML parsing, shared
cross-field checks, and role-specific startup diagnostics. That left common
messages such as unknown fields, invalid ranges, bind failures, and DNS
failures without context.

Added a startup validation section describing the staged checks, representative
constraints and conditional rules, the meaning of check errors versus warnings,
and the limits of a successful environment check. It points operators back to
the field named in each diagnostic and to rerun checks for the relevant binary.
This is documentation only: no runtime, protocol, or performance change is
expected.

## Verification

Cross-checked the documented stages and diagnostic examples against
`parse_config`, `check_local_config`, and `check_remote_config`. No test suite
was run because no code or executable examples changed. Documentation result:
the stated error categories map to current parser/check behavior; no performance
claim applies.
