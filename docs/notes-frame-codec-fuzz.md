# Native frame codec fuzzing

## Scope and approach

The repository already had an isolated `fuzz/` cargo-fuzz package, a
`native_mux_frame` libFuzzer target, and seed corpus entries for valid, truncated,
unknown-kind, and oversized frames. The target previously only called the shape
validator and discarded its result. This change makes valid parses assert the
codec's truncation and trailing-byte invariants, and adds a deterministic unit
regression for those same boundaries. It adds no runtime behavior or dependency
and does not change the wire format or project positioning.

The harness checks every incomplete header length (0 through 8 bytes) and the
last payload truncation boundary for each accepted frame. This follows the
cargo-fuzz/libFuzzer workflow of using mutations to reach parser boundaries;
the project references include transport and mux implementations, but those do
not change the scope of this parser-hardening task.

## Expected benefit

No throughput change is intended. Mutation fuzzing can now detect incorrect
acceptance of truncated input or unstable interpretation when bytes follow a
complete frame, in addition to finding crashes in the existing validator.

## Verification

- `cargo fmt --all` completed successfully.
- `cargo test -p espejismo-core mux::native::frame::tests --offline`: 5 passed,
  including arbitrary-byte panic freedom, frame roundtrips, malformed kind/size,
  and truncation/trailing-byte invariants.
- `cargo test -p espejismo-core --offline`: 142 unit tests and 1 documented
  config integration test passed; doc tests completed.
- Baseline attempt on 2026-10-05 (de): stable is the only installed Rust
  toolchain, `cargo fuzz --version` reports the subcommand is unavailable, and
  the workspace filesystem had only 108 MB free (100% used). Installing
  nightly/cargo-fuzz or building libFuzzer is not viable in this environment.
  Therefore the libFuzzer run count is 0 and executed fuzz cases are 0; these
  are environment measurements, not a no-crash baseline. The required fuzz
  baseline gate remains unmet and must be completed on a machine with nightly,
  cargo-fuzz, and adequate free disk. Do not infer a baseline from the passing
  deterministic regression tests.
- `cargo fmt --all -- --check` found pre-existing formatting differences in
  unrelated client/core/server files; it did not report the changed fuzz
  target, which is outside the workspace.
