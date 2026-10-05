# Reconnect storm tests

## Findings and approach

The client already uses a capped exponential reconnect delay with independent
80–120% random jitter per attempt. That matches the retry-state boundedness
emphasized by the transport references in `docs/research/REFERENCES.md` (notably
quic-go and hysteria2); Espejismo retains its native TCP/yamux tunnel and does
not adopt their transport or positioning. The gap was that existing tests only
checked deterministic delay arithmetic, not whether real jitter sampling
spreads a group of simultaneously failing lanes.

## Changes and expected benefit

Reconnect delay sampling now lives in `sample_reconnect_backoff`, shared by the
runtime wait and the test. A storm regression test samples 256 lanes at the same
failure count and checks every delay remains in the configured jitter bounds,
that at least 20 of 41 integer-millisecond slots are represented, and that the
mean remains close to the nominal delay. This should catch accidental removal
or narrowing of per-attempt jitter that would synchronize reconnect traffic.
Runtime policy and protocol behavior are unchanged; no throughput gain is
claimed or expected. The benefit is bounded, independently jittered retries
remaining verifiable during reconnect storms.

## Verification

Ran `$HOME/.cargo/bin/cargo test -p espejismo-client -p espejismo-server`.
All 63 unit tests passed (38 client, 25 server), including the new 256-lane
sampling test, deterministic exponential-growth/cap/jitter-bound tests, and
server transient-versus-permanent accept retry tests. The storm test exercises
the actual runtime sampler across repeated lane attempts without sleeping or
opening sockets. This is correctness/robustness validation, not a performance
benchmark; no before/after throughput comparison applies because runtime
calculations and retry delays are unchanged.
