# Updater boundary tests

## Analysis and change

The updater only fetches release metadata; it does not download or install
artifacts, verify signatures, or automatically roll back. The requested
signature and rollback cases therefore have no updater code path to exercise.
The scope here is the existing metadata and version-comparison contract.

Numeric dotted versions were compared as integer vectors, which made `1.2`
sort below `1.2.0` despite those tags representing the same numeric release.
The comparison now removes trailing zero components before ordering. Added
boundary tests for equivalent tags, component transitions, malformed/overflow
components, the three documented version field names, and the optional release
URL. The update guide now describes the comparison fallback and the limits of
the metadata check.

Expected gain: accurate update availability for equivalent numeric tag forms;
no runtime or performance claim. Signature verification and automatic
rollback remain unsupported and were not introduced.

## Validation

- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline updater::`: all six
  updater tests pass. An initial run found an incorrect expectation about the
  existing fallback behavior for an overflowing numeric component; corrected
  the test to pin that fallback.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline`: passed, 277 unit
  tests, 10 integration tests, and 1 documentation test; 1 existing
  loopback-dependent test remains ignored as required by the sandbox rule.
