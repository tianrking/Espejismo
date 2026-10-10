# Padding backpressure circuit breaker tests

## Findings and approach

The existing breaker is in core frame writing: a slow write at or above
`backpressure_threshold_ms` temporarily disables optional padding for
`backpressure_cooldown_ms`. Either setting at zero disables the breaker.
This limits extra write pressure while preserving the authenticated frame
format and normal data flow. No lane or connection is rejected.

## Changes and expected benefit

- Added boundary coverage for just below and exactly at the write threshold,
  both zero-valued disable switches, and padding behavior before, during, and
  after cooldown.
- Added an inline comment describing the breaker scope.
- Expected benefit: catch off-by-one threshold errors or accidental permanent
  padding suppression. Runtime behavior and throughput are unchanged.

## Verification

Ran `cargo test -p espejismo-core`: 229 unit tests passed, 1 loopback test was
ignored per the sandbox rule, 9 integration/doc tests passed, and doc tests
passed. The two new `padding_breaker_*` tests cover the exact threshold,
disabled settings, active cooldown, and expired cooldown. No throughput
benchmark applies because behavior and runtime policy are unchanged.
