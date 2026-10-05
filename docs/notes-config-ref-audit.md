# Configuration reference audit

## Audit and change

Compared `docs/deployment/CONFIG.md` with the config schema/defaults in
`crates/espejismo-core/src/config/types.rs` and
`crates/espejismo-core/src/config/defaults.rs`, and checked the server's
fallback activation logic in `crates/espejismo-server/src/main.rs`. The
accepted-parameter reference documented `remote.fallback_http.enabled` only as
a legacy switch without saying what it did. Runtime activation is the logical
OR of `mode = "http_fallback"` and the legacy `enabled` boolean, so setting
`enabled = true` activates fallback even if `mode` remains `"silent"`. Clarified
this precedence and the default silent behavior in the reference.

No runtime or positioning changes. Expected benefit: operators can predict
fallback behavior from the documented values; no performance or resource
improvement is expected.

## Verification

- `cargo test --doc -p espejismo-core`: 1 passed, 0 failed.
- No performance experiment applies to this documentation-only change.
