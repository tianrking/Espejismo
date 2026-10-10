# Connection limit boundary tests

## Findings and approach

The remote accept loop acquires a global `Semaphore` permit with
`try_acquire_owned`. A full semaphore rejects the accepted peer immediately;
the owned permit stays with the peer task and is returned when that task ends.
Existing unit tests covered sequential capacity boundaries and permit release,
but did not exercise simultaneous acquisition attempts. A deterministic
barrier-based unit test can validate the concurrency contract without opening
loopback sockets or changing listener behavior.

## Changes and expected effect

- Added a concurrent acquisition test with 64 tasks racing for 8 permits.
- The test checks the exact admitted and rejected counts, zero remaining
  permits while admitted tasks retain ownership, and full capacity restoration
  after release.
- No runtime behavior or configuration changed. No throughput gain is
  expected; the test guards against future over-admission, incorrect rejection,
  and permit accounting regressions.

## Verification

`$HOME/.cargo/bin/cargo test -p espejismo-server connection_limit_tests --offline`
passed: 4 connection limit tests, including the new 64-attempt concurrent race.
`$HOME/.cargo/bin/cargo test -p espejismo-server --offline` passed: 61 passed,
0 failed, 1 existing loopback-dependent test ignored. Covered branches include
capacity boundaries, exact concurrent admission/rejection counts, permit release,
and post-release capacity reuse. No performance claim applies to this correctness
change.
