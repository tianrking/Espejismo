# Backpressure tests

## Findings and approach

TUN UDP ingress uses two intentional overload policies. A semaphore is acquired
with `try_acquire_owned`, so a full task budget drops newly arriving datagrams
instead of accumulating spawned tasks. Tunnel responses use a bounded Tokio
MPSC queue and await `send`, so producers wait for capacity; if the receiver has
closed, the send returns an error. These are distinct policies and should stay
visible in code and regression tests.

The existing native mux also applies bounded receive windows and send queues.
This follows the bounded-window/resource-control approach used by the `yamux`
reference listed in `docs/research/REFERENCES.md`; no protocol or runtime policy
change is needed for this test-focused task. The change preserves Espejismo's
native encrypted tunnel and small operational model.

## Changes and expected effect

- Documented the non-blocking TUN UDP admission rule beside its semaphore helper:
  saturation drops new datagrams rather than creating unbounded waiting work.
- Added a regression test proving the response queue's awaited send returns an
  error when its consumer has closed. Existing adjacent tests cover a full queue
  causing an awaited sender to block until the consumer frees a slot, and the
  semaphore's full-capacity drop behavior.
- No runtime behavior, throughput, or allocation changes are intended. Expected
  benefit is explicit coverage of the producer wait, overload drop, and consumer
  closure branches for these bounded queues.

## Verification

- `cargo test -p espejismo-client tun::tests`: passed, 4 tests. Covers admission
  through configured semaphore capacity, immediate refusal of excess work and
  permit reuse, full response queue capacity plus awaited send backpressure, and
  send failure after receiver closure.
- `cargo test -p espejismo-client`: passed, 39 tests.
- `cargo test -p espejismo-core mux::native::tests`: passed, 16 tests. Existing
  related coverage includes per-stream send queue pressure, blocked writes when
  the remote window is full, resumption after reads, and aggregate unread-data
  bounds across streams.
- No benchmark was run: this changes test coverage and comments only, with no
  runtime behavior change.
