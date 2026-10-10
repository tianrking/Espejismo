# Configuration Runtime Snapshot Tests

## Findings and approach

`SharedConfig::frame_options` projects configuration into the owned `FrameOptions`
used by the framing runtime. It copies scalar settings and clones the stealth
frame-size candidate list. This is the runtime snapshot boundary: an already
created options value should remain stable while a mutable configuration is
edited, and a later projection should observe the edits. Overrides are applied
when each snapshot is created.

Keep this behavior local to the existing projection. No reload lifecycle,
configuration format, or tunnel protocol behavior changes.

## Change and expected effect

- Documented the ownership and snapshot semantics of `frame_options`.
- Added a regression test that exercises an explicit override, scalar settings,
  the cloned vector, and a second snapshot after source config mutation.
- Expected effect: detect accidental references/shared mutable state or missed
  propagation at this configuration-to-runtime boundary. This correctness-only
  test has no runtime or throughput impact.

## Experiment

- `cargo test -p espejismo-core config::tests::frame_options_are_consistent_runtime_snapshots`: passed (1 targeted test). Covers override precedence, existing snapshot stability after source edits, deep-copy behavior for frame-size candidates, and refreshed values in a subsequent snapshot.
- `cargo test -p espejismo-core`: passed (154 unit tests, 1 config example integration test, 4 HTTP proxy integration tests, and 1 doctest; 0 failures).
- `cargo fmt --all`: completed.
- No throughput benchmark applies to this correctness-only change.
