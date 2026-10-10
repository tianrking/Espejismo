# Authentication attempt throttling coverage

## Findings and approach

The remote handler already routes failed core handshakes through
`fallback_or_reject`. When no HTTP fallback is enabled, a configured nonzero
`reject_delay_ms` keeps the failed connection open silently for that delay and
then closes it. With zero delay, `TarpitManager` retains rejected TCP streams
up to its configured capacity and evicts the oldest held stream when full.
There is no per-source failure counter or timed IP ban in the current runtime.

This task adds deterministic regression coverage around the existing
authentication rejection delay. It deliberately does not introduce a new
per-IP ban policy, which would require choices about identity (especially
behind proxies), thresholds, expiry, and bounded state.

## Changes and expected effect

- Generalized the silent delayed-reject helper over `AsyncWrite`, preserving
  runtime behavior while allowing an in-memory duplex stream in the test.
- Added a test that proves failed-auth rejection waits for the configured
  interval before shutting down the stream.
- No throughput gain is expected; the benefit is regression coverage for the
  authentication failure throttle already exposed by `reject_delay_ms`.

## Verification

`$HOME/.cargo/bin/cargo test -p espejismo-server -p espejismo-core` passed.
This includes the new delayed-rejection branch and existing handshake/auth
tests in core, plus the server rejection and connection-limit tests. The test
uses Tokio's in-memory duplex stream because sandbox networking denies local
TCP binds; the initial socket-based version failed with `PermissionDenied` and
was replaced. No performance experiment applies to this correctness change.

## Scope note

Repeated failed connections can be delayed or placed in the bounded silent
tarpit, but are not banned by source address. The requested continuous-failure
IP ban semantics remain unspecified and unchanged.
