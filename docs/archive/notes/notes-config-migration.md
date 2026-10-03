# Configuration migration diagnostics

## Analysis and changes

Config tables reject unknown keys so an option removed or renamed between
releases cannot silently fall back to a default. The existing parser already
reports the unknown key and offers a close-match suggestion, but when an old
key has no similar replacement the error did not explain that it could be a
release migration issue. Extend that diagnostic to tell operators to check
whether the option was renamed or removed. This preserves strict parsing and
all supported legacy fields (including `remote.egress.socks5_proxy`) without
guessing aliases or changing config semantics. The expected benefit is faster
identification of stale config after upgrades; no performance gain is expected.

## Verification

- `cargo test -p espejismo-core`: 123 unit tests, 1 config-example integration
  test, and 1 doctest passed; 0 failed.
- Regression coverage checks that an unknown option retains its field name and
  includes migration guidance; existing typo-suggestion tests remain in place.
- Correctness result: migration diagnostic regression and the full core suite
  pass. No performance claim applies to this validation-only change.
