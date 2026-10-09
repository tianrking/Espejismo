# Runtime state boundary tests

## Findings and approach

`RuntimeState` is a mutex-protected status snapshot, not a transition-enforcing
state machine. Callers intentionally provide status strings through
`set_tunnel_state`, while `record_connect_success` and `record_error` apply the
counter transitions atomically. Adding a closed enum or rejecting unknown
strings would change the public snapshot/API contract, so this change tests the
existing behavior instead: counters reset/increment at the right transitions,
recent errors remain bounded and ordered, concurrent updates are serialized,
and counters saturate at `u64::MAX`.

The recent-error helper previously drained into a temporary `VecDeque` and
converted back to `Vec` for each error. It now removes the oldest entry from the
already bounded vector and appends the new one. This preserves the serialized
snapshot shape and avoids rebuilding the queue; no throughput claim is made.

## Expected impact

No externally visible lifecycle behavior changes. Runtime-state updates for
errors avoid a temporary queue allocation and conversion. The new tests pin the
current transition and concurrency guarantees, making accidental counter loss,
overflow wraparound, or error-history growth/order changes detectable.

## Verification

- `cargo test -p espejismo-core runtime_state::tests --offline` — 6 passed.
- `cargo test -p espejismo-core --offline` — 265 unit tests passed, 1 ignored
  loopback test; 1 config example, 10 HTTP proxy tests, and 1 doctest passed.
- These unit tests do not bind or connect loopback sockets.
- The first saturation-test draft failed because it asserted a failure count
  after the successful-connect transition, which correctly resets that count.
  The test now checks reconnect saturation, then separately seeds and checks
  failure saturation; both the focused and full crate runs pass.
