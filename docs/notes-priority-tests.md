# Stream priority boundary tests

## Findings and plan

The native mux already gives control frames precedence and limits a sustained
interactive burst to eight data frames while bulk remains queued. Existing
tests covered the common ninth-frame handoff, but did not pin down the exact
threshold when bulk joins after idle interactive traffic, whether control
frames alter the burst count, or the accepted wire values at the enum edges.
Add deterministic tests around those boundaries without changing scheduling
or protocol behavior. This preserves the project's existing two-class model
and native encrypted tunnel positioning.

## Changes and expected effect

- Added exact-threshold tests for delayed bulk arrival, control-frame
  precedence, and a fresh fairness period after queues drain.
- Added valid/invalid `StreamPriority` wire-value boundary coverage.
- Updated QoS documentation to state the eight-data-frame bound and clarify
  that control frames do not consume it.
- Expected runtime/performance change: none; these tests codify existing
  behavior. The tests prevent starvation-bound and wire-compatibility
  regressions.

## Validation

`$HOME/.cargo/bin/cargo test -p espejismo-core --offline` passed: 213 unit
tests, 1 config-example integration test, 8 HTTP proxy integration tests, and
1 doc test (223 passed); 1 loopback-bind test remained ignored under the
documented sandbox restriction. New coverage exercised the inclusive
eight-frame boundary with delayed bulk arrival, control-frame precedence
without burst-counter consumption, a new contention period after draining,
and valid values 1/2 versus invalid priority bytes 0/3/255. No scheduling or
wire behavior changed, so no performance gain is claimed. `cargo fmt --all
-- --check` was not clean because unrelated existing files also differ from
rustfmt output; the two modified Rust files were formatted directly.
