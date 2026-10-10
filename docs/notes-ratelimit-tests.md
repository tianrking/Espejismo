# Rate limit test coverage

## Findings and approach

The server's per-user byte quota is a fixed-duration counter window: it starts
when `UserLimitRegistry` is created and resets after `quota_window`. The user
guide called this a rolling window, which did not match the implementation.
The encrypted transport's `StealthPaddingBudget` is a token bucket with a burst
of at least two configured frame sizes. Following the small, bounded state
machines used for pacing in quic-go (see `docs/research/REFERENCES.md`), this
change adds deterministic unit coverage of token consumption/refill and quota
window rollover without changing protocols or limiter behavior.

## Changes and expected effect

- Added tests for padding token exhaustion, refill after elapsed time, and the
  disabled-budget path.
- Added tests for quota exhaustion, reset after the configured window elapses,
  and zero-byte accounting.
- Clarified in `docs/deployment/USERS.md` that quota windows are fixed-duration
  windows. Added a code comment identifying the padding budget as a token
  bucket.

These are correctness and regression tests, so no throughput gain is expected;
the expected improvement is earlier detection of regressions in quota rollover
and token refill behavior. Runtime limiter behavior and configuration are
unchanged.

## Verification

`cargo test -p espejismo-server -p espejismo-core` passed: core 150 unit tests,
1 config-example integration test, 4 HTTP proxy integration tests, server 25
unit tests, and 1 core doctest. New covered branches include exhausted/refilled/
disabled token budget and exhausted/rolled-over quota, including zero-byte
accounting. An initial version of the refill assertion exposed that the bucket
burst is at least two frames (not equal to the configured rate); the test was
corrected to match the documented burst rule and the full command passed.

`cargo fmt --all -- --check` reports formatting differences in unrelated
existing files across the workspace. `rustfmt --edition 2021 --check` on the
two changed Rust files reports only pre-existing formatting differences later
in `transport/mod.rs`; the added sections are formatted.
