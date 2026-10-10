# Connection migration recovery

## Findings and approach

The client uses long-lived TCP transports carrying authenticated frames and a
multiplexed session. TCP connections cannot migrate between network interfaces,
and the current protocol has no replicated mux state or stream-resume token.
After a network change, an existing stream can therefore fail; the supported
recovery boundary is opening a new mux stream after the failed physical session
is discarded and a fresh authenticated connection is established. This matches
the project's simple TCP tunnel model and does not add protocol camouflage or a
new transport.

The lane connection-success bookkeeping is now centralized. A reconnect marks
one session rotation, refreshes the connection timestamp, clears consecutive
failures and stale error details, and updates activity atomically as one state
transition. This makes network-switch recovery visible and prevents stale
failure state from surviving a successful new connection.

## Expected benefit

Correctness and observability improvement; no throughput change is expected.
This does not resume bytes or preserve already-open streams across a network
switch. New streams use the existing reconnect path and fresh authentication.

## Verification

- `cargo test -p espejismo-client network_reconnect_records_rotation_and_resets_failure_state`
  covers initial connection accounting, a failed old path, and successful
  reconnect cleanup/rotation accounting without binding a loopback socket.
- Full `cargo test -p espejismo-client` passed: 49 tests, 0 failed, 0 ignored.
- No throughput benchmark was run because this is a correctness/accounting
  change with no intended performance effect.
