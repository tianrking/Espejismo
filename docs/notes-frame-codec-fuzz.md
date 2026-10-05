# Native frame codec fuzzing

## Scope and approach

The repository already had an isolated `fuzz/` cargo-fuzz package, a
`native_mux_frame` libFuzzer target, and seed corpus entries for valid, truncated,
unknown-kind, and oversized frames. The target previously only called the shape
validator and discarded its result. This change makes valid parses assert the
codec's truncation and trailing-byte invariants, and adds a deterministic unit
regression for those same boundaries. It adds no runtime behavior or dependency
and does not change the wire format or project positioning.

The fuzzing pattern follows Rust's cargo-fuzz/libFuzzer workflow; the project
references include transport and mux implementations, but those do not change
the scope of this parser-hardening task.

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
- Fuzz execution was blocked. `cargo fuzz` is not installed, and
  `cargo build --manifest-path fuzz/Cargo.toml --bin native_mux_frame --offline`
  could not resolve `libfuzzer-sys` from the local cache. Retrying without
  `--offline` failed because `index.crates.io` could not resolve. Consequently
  no libFuzzer run count or no-crash baseline is claimed, and the fuzz baseline
  gate remains unmet.
