# Token bucket burst coverage

## Findings and approach

`StealthPaddingBudget` in `crates/espejismo-core/src/transport/mod.rs` already
uses a token bucket for optional idle padding. It starts full, refills at the
configured byte rate, and clamps credit to a finite burst of at least two
stealth frames. Existing tests covered startup exhaustion, refill, and the
disabled path, but did not prove that a long idle interval cannot accumulate
credit above that cap. The references review found no need to import another
project's implementation: the existing small state machine fits the project's
minimal transport model and changes no protocol or camouflage behavior.

## Changes and expected effect

- Extracted a private `refill_at(Instant)` operation so refill timing and burst
  capping can be checked deterministically without sleeping.
- Added a regression test that drains initial credit, simulates 30 seconds idle,
  and verifies the available credit is still exactly the configured burst.
- Documented the initial-full and maximum-burst behavior alongside the
  `padding_budget_bps` setting.

This is a correctness/robustness change. It should not affect throughput or
runtime behavior; it ensures idle padding remains bounded after long pauses.

## Verification

`$HOME/.cargo/bin/cargo test --offline -p espejismo-core stealth_padding_budget`
passed all 4 matching tests, including initial burst exhaustion, elapsed refill,
disabled behavior, and the new long-idle cap case. The full
`$HOME/.cargo/bin/cargo test --offline -p espejismo-core` run passed: 181 unit
tests, 1 config example integration test, 4 HTTP proxy integration tests, and 1
doctest. The change is correctness-only; no throughput gain is claimed or
expected.
