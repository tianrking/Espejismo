# Frame codec property tests

## Scope and approach

`crates/espejismo-core/src/mux/native/frame.rs` already had seeded random loops and a
fuzz-validation entry point. Replace the ad hoc random loops with `proptest` cases
for valid asynchronous frame roundtrips, complete header/payload validation, and
panic-free arbitrary byte input. Keep explicit malformed-header checks for unknown
frame kinds and payloads above `MAX_PAYLOAD`.

This follows the property-based testing approach used across Rust protocol
implementations: generate many values from constrained valid domains, while
letting the runner shrink failures. It does not alter the wire format or project
positioning.

## Expected benefit

No runtime performance change is intended. The test suite should explore more
combinations than the former fixed-seed loops and produce smaller counterexamples
when a codec invariant fails.

## Verification

- `cargo fmt --all` completed successfully.
- `cargo test -p espejismo-core mux::native::frame::tests --offline` could not
  resolve `proptest`: it is absent from the local Cargo cache.
- An online Cargo attempt also failed because `index.crates.io` could not be
  resolved in this environment.
- Therefore compilation and test results are unavailable; no claim of passing
  tests or regression-free behavior is made. The required validation gate remains
  unmet, so there is no `.commit-msg` for this change.
