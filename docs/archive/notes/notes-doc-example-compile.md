# Documented Configuration Examples

## Analysis and changes

The core crate already had a doctest for the maintained full configuration
file, but standalone TOML examples in the README and deployment/testing docs
were not checked. Added an integration test that discovers fenced `toml`
blocks in both READMEs and `docs/`, then validates each block with the same
`parse_config` function used by the binaries. This catches TOML syntax,
schema, and cross-field validation errors without requiring network or service
startup.

The first run found an invalid lane example in
`docs/testing/THROUGHPUT_TUNING_HK2_RK.md`: it requested five lanes while the
default maximum was four. The example now sets `max_connections = 5`.

Expected benefit: all currently fenced configuration snippets in the scanned
documentation remain parseable as project configs, and future invalid edits
fail `cargo test -p espejismo-core`. This adds correctness coverage, with no
runtime or performance change expected.

## Verification

- `cargo test -p espejismo-core --test doc_config_examples`: passed; all TOML
  fences in `README.md`, `README_ES.md`, and `docs/` parsed successfully after
  correcting the lane example.
- `cargo test -p espejismo-core`: passed; 120 unit tests, the documentation
  example integration test, and the maintained-config doctest all passed.
