# Configuration examples index

## Analysis and changes

Before this change, the operations index linked to the configuration reference,
but there was no page to help readers choose among the maintained full TOML
file and the complete scenario snippets embedded in that reference. The
examples were available, but operators had to read through the long reference
to discover which scenario suited their deployment.

Added `docs/deployment/CONFIG-EXAMPLES.md` to catalog the maintained one-file
config and four documented starting points: minimal server/client, one-user
SOCKS5, multi-user limits, and TUN route/DNS takeover. Linked the index from
the operations index, configuration reference, and README navigation.

Expected benefit: reduce the time spent locating a suitable configuration by
providing direct paths by deployment need. This is a documentation-only
usability change; runtime behavior, dependencies, and performance are
unchanged, so no measurable runtime percentage is expected. No positioning
change: the examples continue to use the existing two-binary, one-TOML model.

## Verification

- `cargo test --offline -p espejismo-core --test doc_config_examples`: passed
  (1 test; all documented TOML examples parsed successfully).
- No performance experiment applies to this documentation-only change.
