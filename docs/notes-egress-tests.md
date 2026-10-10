# Egress connection boundary tests

## Findings and approach

The raw TCP egress path already applied a five second timeout to each resolved
address and continued after failures. However, the timeout lived inside the
production connector closure, so the address-selection test could not exercise
the timeout branch. Following the bounded-attempt approach used by resilient
connectors such as Hysteria2 and quic-go (see `docs/research/REFERENCES.md`),
the per-address timeout is now an explicit input to the tested selection helper.
This retains sequential raw TCP egress and the existing five second policy.

## Changes and expected benefit

- Moved timeout enforcement around each connector attempt in
  `connect_first_allowed`; production still uses five seconds and timeout
  continues to the next policy-allowed DNS address.
- Added tests for timeout then success, immediate failure then later failure
  reporting, empty DNS results, and policy rejection without an attempt.
- Expected benefit: protect the existing failover and error-reporting boundary
  from regressions. No throughput change is expected; successful connects use
  the same socket operation and timeout budget.

## Verification

Ran `cargo test -p espejismo-server --offline`: 63 passed, 0 failed, 1 ignored.
Tests cover simulated failure, timeout advancement, last-error selection, empty
results, policy filtering, and the existing ignored SOCKS5 loopback relay test. The ignored test requires
loopback bind and is run outside the sandbox with
`cargo test -p espejismo-server -- --ignored`.

Conclusion: reliability boundary coverage expanded; no runtime policy or
throughput claim changed.
