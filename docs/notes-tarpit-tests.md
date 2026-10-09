# Tarpit boundary tests

## Findings and approach

`TarpitManager` already used a bounded channel and evicted the oldest held
socket when its entry capacity filled. The worker's retention queue also
respected `tarpit_max`, but a zero-capacity configuration could transiently
buffer one socket in the channel, and the fixed five-second expiry sweep could
retain sockets well beyond a short configured hold. The architecture notes
describe the pool's capacity and TTL guarantees; the sweep cadence was not
documented.

The implementation now bypasses enqueueing when capacity is zero or hold is
zero, rejects zero-capacity insertions in its queue policy, and sweeps at
`min(hold_for, 5s)`, with a 1ms floor to avoid a zero-duration timer. This keeps
the existing silent-reject design and oldest-entry eviction.
The queue policy is generic and tested without sockets, so these regression
tests run under the restricted loopback sandbox. This follows the resource
bound approach documented by the project and the bounded-state practices used
by Rust async networking implementations such as quic-go; no protocol or
product-position change is involved. See `docs/research/REFERENCES.md` and
[quic-go](https://github.com/quic-go/quic-go).

## Expected effect

For holds below five seconds, expiry cleanup now runs at the configured hold
cadence (subject to Tokio scheduling and the 1ms floor), reducing stale socket
retention from as much as roughly five seconds to roughly one hold interval.
Retained queue occupancy remains at or below the configured capacity,
including capacity zero; a zero-capacity manager drops sockets immediately.
No throughput claim applies to this correctness/resource-bound change.

## Verification

Ran `cargo test -p espejismo-server --offline`: 60 passed, 0 failed, 1 ignored.
The ignored test is `relay::tests::relays_tcp_through_two_socks5_hops` and
requires loopback bind; the tarpit tests themselves all ran. New regression
coverage checks zero capacity, oldest-entry eviction at the capacity boundary,
and expiry of dead entries while retaining a live entry. No performance
benchmark was run because this is not a throughput optimization. The existing
loopback test remains ignored for the sandbox and is covered by the documented
out-of-sandbox `cargo test -- --ignored` procedure.
