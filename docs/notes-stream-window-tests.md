# Stream window boundary tests

## Scope and rationale

The stream receive-credit calculation subtracts buffered bytes and outstanding
credit from the configured window ceiling using checked arithmetic. Existing
coverage exercised conversion overflow, accumulated-buffer overflow, invalid
available credit, even and odd update thresholds, and the configured ceiling.
The `u32::MAX` ceiling itself was not pinned by a deterministic boundary test.

The reference list identifies HashiCorp Yamux as the project to consult for
window management. This work preserves its existing credit-accounting model and
the project's native encrypted tunnel positioning; it adds no protocol or
configuration changes.

## Change and expected effect

Added a unit test for exact maximum credit, one byte below the maximum, the
buffered-byte subtraction at the maximum, and invalid buffered credit when the
receive window is already full. These cases verify that upper-bound arithmetic
returns the exact delta or rejects invalid state instead of wrapping. This is
correctness coverage; no performance change is expected or claimed.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p tokio-yamux -- --test-threads=1`:
  47 unit tests passed, 0 failed; the loopback integration test was skipped as
  ignored (`requires loopback bind`); doc-tests passed (0 tests). The unit suite
  includes the new upper-bound test and existing threshold, accounting,
  receive-overrun, and send-overflow coverage.
- The sandbox loopback restriction applies to the ignored integration test;
  no assertion failure occurred. No throughput benchmark was run because this
  is a correctness-only test change with no performance claim.

## Conclusion

The `u32::MAX` receive-window boundary now has deterministic regression
coverage, and the complete non-ignored `tokio-yamux` test suite passes with no
regression observed.
