# Retry backoff tests

## Findings and approach

The server listener retries only recognized temporary resource exhaustion errors;
its exponential delay starts at 250 ms and saturates at 16 seconds. The client
lane reconnect delay uses a 250 ms exponential base, an 80–120% jitter window,
and a 16 second ceiling. Both paths already had example-value tests, but those
examples did not exercise the delay invariants across the full saturated range.

Following the repository's reference guidance on explicit retry state bounds
(see `docs/research/REFERENCES.md`, including quic-go and hysteria2's retry and
loss-recovery engineering), this change keeps the existing simple TCP listener
and reconnect policies and adds deterministic boundary/invariant checks. No
protocol, deployment model, or runtime behavior changes.

## Changes and expected benefit

- Server accept backoff tests now check monotonicity through 32 failures and
  verify the delay remains exactly capped at 16 seconds after saturation.
- Client reconnect backoff tests compare 80%, 100%, and 120% jitter for failure
  counts 1–128 and cover `u32::MAX` arithmetic saturation.
- Expected benefit: catch regressions in cap, monotonicity, jitter ordering, or
  large-counter overflow without timing-sensitive sleeps. Runtime performance
  is unchanged; this is a correctness and robustness change.

## Verification

Ran `$HOME/.cargo/bin/cargo test -p espejismo-server -p espejismo-client`.
Result: all 52 unit tests passed (32 client, 20 server); both new invariant
tests passed alongside the existing known-transient/permanent accept-error and
backoff example tests. The tests cover ordinary growth, saturation, maximum
failure counters, and the full supported client jitter range. No benchmark was
run because this change adds test coverage only and does not alter runtime work.
