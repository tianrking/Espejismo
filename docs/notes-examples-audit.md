# Configuration example audit

## Analysis and changes

The repository already has an integration test that extracts every fenced
`toml` block from `README.md`, `README_ES.md`, and Markdown files under
`docs/`, then passes each block to `espejismo_core::parse_config`. The
maintained `configs/examples/espejismo.toml` is separately parsed and
serialized by the core crate documentation example. The independent
`local-configs/espejismo.toml` template is not in either automatic scan, so it
was checked through the local binary's `--check-config` entry point.

No example required a documentation correction. This round records the audit
scope and results so future edits can distinguish TOML parsing from checks
that depend on a host's interfaces, sockets, DNS, or deployment values. The
change is documentation-only; expected parser correctness improvement is
maintaining coverage rather than changing runtime behavior. No performance
improvement is expected.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core --test doc_config_examples`:
  passed (1 test), parsing all fenced TOML examples in the scanned Markdown.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --doc`: passed (1 doctest),
  including parse/serialize/parse of the maintained full example.
- `espejismo-local --check-config` read both `local-configs/espejismo.toml`
  and `configs/examples/espejismo.toml`; parsing and PSK checks proceeded.
  The command exited nonzero because the local template contains the
  placeholder host `xxx.xxx.xxx.xxx`, and listener socket creation is denied
  in this environment.
- `espejismo-remote --check-config` read both standalone TOML files and
  validated their user/PSK data. The command exited nonzero because listener
  socket creation is denied in this environment.
- No performance experiment applies to this documentation-only audit. No
  parser failures or malformed documented TOML examples were found.
