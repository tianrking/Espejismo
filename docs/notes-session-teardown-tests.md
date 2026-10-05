# Native mux session teardown tests

## Findings and approach

The native mux tracks each stream's send window and blocked writer waker in a
shared flow state. When the session loop ended after transport EOF, it dropped
the stream table without closing those flow states. A `NativeStream` kept by
the application could therefore remain pending forever while waiting for
window credit from a session that no longer existed. The frame reader also ran
as a detached task; aborting or ending the parent session could leave it
holding the transport read half while waiting for another frame.

The `yamux` reference in `docs/research/REFERENCES.md` calls out explicit
session keepalive and lifecycle handling. This change applies that lifecycle
discipline to the existing native mux while preserving its framing, drain
policy, and transport positioning.

## Changes and expected benefit

- Closing the session loop closes every remaining stream flow state, waking
  blocked writers so they return `BrokenPipe` after teardown.
- The frame reader is now held by an abort-on-drop guard and explicitly
  stopped when the session exits, so the read half is reclaimed with the
  session even when the parent task is aborted.
- Added regression tests for abrupt transport disconnect and GOAWAY drain
  timeout while a stream writer is blocked on flow control.
- Expected benefit: bounded teardown and no permanently pending write or
  detached reader after session termination. This is a correctness and
  resource reclamation change; throughput improvement is not expected.

## Verification

- Both focused regressions passed:
  `cargo test -p espejismo-core native_mux_goaway_timeout_wakes_blocked_stream_writer`
  and
  `cargo test -p espejismo-core native_mux_transport_disconnect_wakes_blocked_stream_writer`.
  They verify that drain timeout and remote transport EOF wake a write blocked
  at the send window with `BrokenPipe`.
- `cargo test -p espejismo-core` passed: 156 unit tests, 1 config example
  integration test, 4 HTTP proxy integration tests, and 1 doc test.
- No throughput benchmark was run because this is a correctness and resource
  cleanup change with no intended performance change.
