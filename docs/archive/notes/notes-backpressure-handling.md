# Backpressure handling

## Findings and approach

The TUN UDP response writer consumes completed relay responses through a
Tokio unbounded channel. Relay task creation is capped at 1024 permits, but a
slow or stalled netstack writer can still retain many completed response
vectors without waiting for channel capacity. Use a bounded channel with the
same capacity as the task semaphore. A relay task awaits queue capacity while
holding its permit, so queued and waiting responses remain covered by the
existing concurrency ceiling. If the writer exits, senders observe channel
closure and release their responses.

This follows the bounded-queue/backpressure approach used in Tokio async
pipelines and keeps the existing TUN and authenticated tunnel design intact.
The local reference list points to shadowsocks-rust for async UDP relay
patterns; no protocol or dependency changes are needed here. tokio-yamux has a
separate unbounded stream-event queue whose local send window and configured
stream limits are documented in its session code; changing that queue's
protocol behavior is outside this focused fix.

## Expected effect

The response queue can no longer grow without bound under a slow consumer.
At most 1024 relay operations hold permits at once, covering both queued and
blocked response senders. No throughput increase is expected for healthy
consumers; under overload, producers wait and memory use stays constrained by
the configured concurrency and maximum UDP payload sizes.

## Validation

`$HOME/.cargo/bin/cargo test -p espejismo-client tun::tests` passed (1 passed).
`$HOME/.cargo/bin/cargo test -p espejismo-client` passed (30 passed, 0 failed).
The regression test verifies a full response queue blocks a producer until the
consumer drains an entry. This is a correctness and memory-bound improvement;
no throughput benchmark was run because healthy-consumer throughput is not the
expected change.
