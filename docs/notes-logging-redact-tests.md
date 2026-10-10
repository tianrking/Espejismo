# Logging redaction boundary tests

## Findings and approach

The shared logger writes `tracing` events as compact, pretty, or JSON output and
does not inspect or redact arbitrary event fields. Secret safety therefore
depends on credential-bearing values having safe `Debug` implementations before
they are attached to an event. The handshake configuration masks its PSK and
derived authentication key; local proxy authentication masks both username and
password. Existing tests checked those types separately, but did not tie that
boundary to the logging module.

Add a logging-focused regression test that formats both secret-bearing values
through the same `Debug` representation used by tracing shorthand fields, and
asserts that the PSK, user name, and token-like password never appear. This is a
test-only hardening change; it does not change output behavior or add runtime
work. Expected operational benefit is preventing regressions that could write
these values to configured log files. No throughput change is expected.

## Verification

- `cargo test -p espejismo-core --offline` — passed: 278 unit tests passed,
  1 existing unit test ignored, 11 integration tests passed, and 1 doctest passed.
- The focused regression test covers PSK, derived authentication key, username,
  and password/token redaction at the `Debug` logging boundary. The test is
  in-memory and does not bind loopback sockets.
- No performance benchmark applies; no runtime code changed, so there is no
  performance impact.
