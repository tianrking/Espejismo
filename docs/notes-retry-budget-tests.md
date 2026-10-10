# Retry budget boundary tests

## Findings and approach

DNS resolution uses a 10 second overall timeout, at most three resolver attempts,
and waits of 100 ms and 250 ms after the first and second failures. The existing
retry loop had success and attempt-limit coverage, but its exact wait budget and
no-wait final-attempt boundary were implicit. Following the bounded retry-state
approach referenced in `docs/research/REFERENCES.md` (quic-go and Hysteria2),
this change makes the delay lookup explicit and tests its boundary without
changing the resolver model or Espejismo's positioning.

## Changes and expected benefit

- Added `dns_retry_delay_after_failure`, used by the existing loop to select
  whether another attempt has a scheduled wait.
- Added a deterministic boundary test for the 100 ms and 250 ms delays, no delay
  after the last permitted attempt, an extreme attempt index, and the 350 ms
  aggregate retry wait remaining below the 10 second overall timeout.
- Expected benefit: fail quickly and locally if attempt indexing, delay budget,
  or the terminal retry boundary regresses. Runtime policy and performance are
  unchanged.

## Verification

Ran `$HOME/.cargo/bin/cargo test -p espejismo-core`. The new
`dns_retry_wait_budget_has_no_delay_after_final_attempt` test passed, alongside
existing DNS retry success, attempt exhaustion, and timeout tests. The core test
suite passed with its loopback-dependent test ignored by the repository's
sandbox convention. No throughput benchmark applies to a correctness-only test
and helper refactor with no runtime policy change.
