# Configuration Robustness Tests

## Plan and expected outcome

- Add regression coverage at the TOML boundary for malformed syntax, omitted
  sections/fields, and a value with the wrong TOML type.
- Keep parser behavior unchanged: TOML already reports source locations and
  Serde rejects incompatible values; validation errors name the config field.
- Document defaulting and diagnostics in `docs/deployment/CONFIG.md`.
- Expected improvement: catch regressions that make malformed or incomplete
  configs hard to diagnose, with no runtime or performance change expected.

## Validation

- `cargo test -p espejismo-core config::tests:: --lib` initially caught an
  overly broad test expectation (`integer`); the parser reports the concrete
  type `u32`, so the assertion now checks that stable field/type detail.
- `$HOME/.cargo/bin/cargo test -p espejismo-core`: passed (139 unit tests and
  the package integration/doc example targets). New cases verify a syntax
  error's line/column, omitted section and field defaults, and the field,
  expected type, and line in a type mismatch. Existing tests continue to cover
  invalid semantic values and their field-specific explanations.
- This is a correctness and diagnosability change; no performance result
  applies and no runtime behavior changed.
