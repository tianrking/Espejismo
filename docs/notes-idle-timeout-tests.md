# Idle timeout boundary tests

## Findings and approach

The TCP relay's `idle_copy_bidirectional` wraps each pending read in Tokio's
idle timeout. Existing in-memory duplex tests covered an ordinary idle expiry,
traffic extending the deadline, and half-close behavior. The zero-duration
boundary was not explicit. Add a deterministic duplex-stream regression for
that boundary; it requires no loopback sockets and does not change timeout
policy. This stays consistent with the timeout-focused state-machine testing
used by yamux and does not alter Espejismo's positioning or wire protocol.

## Changes and expected benefit

- Add coverage proving a zero-duration idle limit ends pending reads and
  returns zero copied bytes.
- Expected benefit: prevent regressions in the immediate-expiry edge case; no
  throughput gain is expected because this is a correctness test only.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline idle_copy_bidirectional` passed: 5 passed, 0 failed. This covers ordinary idle expiry, zero-duration immediate expiry, traffic refreshing the timeout, one-direction half-close, and read errors using in-memory duplex streams.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline` passed: 209 unit tests, 1 ignored (loopback bind), 8 HTTP proxy integration tests, 1 config example test, and 1 doctest; 0 failures.
- No throughput benchmark was run because this is correctness-only and changes no runtime behavior.
