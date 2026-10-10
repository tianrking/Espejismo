# Transport retry boundary tests

## Findings and approach

The client reconnect delay is capped exponential backoff with an independent
80–120% jitter sample; server accept retries for recognized temporary resource
exhaustion use a capped 250 ms exponential delay. The transport references in
`docs/research/REFERENCES.md` (quic-go and hysteria2) support bounded retry
state and backoff. This task retains the existing native TCP/yamux protocol and
deployment model, and adds no retry sleeps to tests.

## Changes and expected benefit

- Centralized the client's minimum reconnect attempt count (`max(1)`) in
  `reconnect_attempt_limit` and added boundary coverage for configured counts
  0, 1, 2, and `u32::MAX`. Both client initialization and stream retry use the
  same rule.
- Existing tests cover client exponential growth, 16-second ceiling, 80/100/120
  percent ordering through failure count 128 and `u32::MAX`, and actual jitter
  sampling across lanes. Existing server tests cover accept delay growth,
  saturation and permanent-error non-retry behavior.
- Expected benefit: prevent off-by-one/minimum-attempt regressions and catch
  retry delay cap or jitter-bound regressions without wall-clock timing
  dependence. Runtime performance and protocol behavior are unchanged.

## Verification

- `cargo test -p espejismo-client --offline`: passed, 54 tests; includes the
  new attempt-limit boundary test, full backoff/jitter bounds, randomized
  sampler bounds and reconnect state reset.
- `cargo test -p espejismo-server --offline`: passed, 63 tests, 1 ignored
  loopback-only test; includes accept backoff saturation and transient versus
  permanent error classification.
- The ignored server test is an existing loopback-dependent relay integration
  test. No new socket test was introduced. No benchmark applies because this
  is a testability/correctness change and runtime retry timing is unchanged.
