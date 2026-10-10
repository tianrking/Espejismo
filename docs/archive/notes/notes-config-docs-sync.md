# Configuration defaults and documentation sync

## Audit and change

Compared the configuration reference in `docs/deployment/CONFIG.md` with the
Serde defaults and `Default` implementations in
`crates/espejismo-core/src/config/types.rs`, `config/defaults.rs`, and the
framing/underlay constants they use. The field descriptions covered the schema,
but many omitted fields did not say what happens when they are absent. Added a
complete omitted-field default table to the config reference, including unset
optional values, enum defaults, and the default values for nested tables.

The table describes parser defaults only. Named profiles intentionally overlay
some settings, so they remain documented separately. No configuration behavior,
runtime code, or project positioning changed.

## Expected benefit

Operators can determine the effective baseline without inferring Serde behavior
or searching implementation files. This is a documentation accuracy and
diagnosis improvement; no throughput or runtime improvement is expected.

## Verification

- Compared every configuration field in the schema with the corresponding
  serde default, manual `Default` implementation, enum default, or shared
  constant. Corrected the stealth tick entry to `50` ms from
  `DEFAULT_STEALTH_TICK_MS`.
- `$HOME/.cargo/bin/cargo test --doc -p espejismo-core`: 1 passed, 0 failed.
- No performance experiment applies to this documentation-only change.
