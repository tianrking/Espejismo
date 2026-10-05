# Unknown configuration keys

## Analysis and changes

TOML deserialization previously ignored unknown keys, so a typo or a removed
option could silently leave a default in effect. The config table structs now
reject unknown fields, including nested tables. `parse_config` turns the
deserializer's error into a concise `unknown config field` diagnostic and adds
a `did you mean` suggestion when a known key is sufficiently close. Unknown
keys without a close match still name the offending key. This keeps the
existing parse and `--check-config` flow authoritative rather than adding a
warning channel that callers could miss. No dependency or runtime protocol
changes; the expected gain is catching configuration mistakes before startup.

## Verification

- `cargo test -p espejismo-core`: 114 passed, 0 failed.
- Regression tests cover a top-level table typo (`max_stream` → `max_streams`)
  and a nested table typo (`enabeld` → `enabled`).
- Correctness result: unknown keys now fail with actionable diagnostics;
  existing core tests, including config round trips, pass. No performance
  claim applies to this validation change.
