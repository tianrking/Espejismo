# docs-doctests

## Scope and rationale

- `docs/` contains operational shell commands, TOML configuration, protocol
  diagrams, and prose; it has no Rust code fences to convert directly into
  doctests. Turning shell or TOML examples into Rust would validate a different
  thing.
- Added a crate-level `espejismo-core` doctest that includes the maintained
  `configs/examples/espejismo.toml`, parses it, serializes it, parses it again,
  and checks a representative setting. Linked this check from the config guide.
- This checks real documentation input with the production config parser while
  remaining local, deterministic, and network-free. No runtime or protocol
  behavior changes; expected performance change is 0%.

## Validation

- `cargo test --doc --workspace`: passed. `espejismo-core` ran 1 doctest and
  `tokio-yamux` ran 0; all passed. No network was used.
- Outcome: the maintained full TOML example compiles through the documented
  parse/serialize API with no regression. Documentation/test-only change; no
  performance experiment applies.
- Expected benefit: prevent drift between the checked-in full config example
  and the public config parse/serialize API. No quantified operator time saving
  is claimed.
