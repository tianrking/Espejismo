# Session migration regression coverage

## Scope and findings

The requested session migration state-preservation test maps to a documented boundary in this codebase. `docs/ARCHITECTURE.md` says that loss of a physical tunnel terminates its mux streams and that transparent stream migration/replay is not implemented; the future connection manager must own that behavior. The production mux default remains yamux, while this focused regression targets the native mux session lifecycle. This change does not claim or add transparent migration.

The implementation follows the existing mux isolation model: a replacement physical session starts with fresh stream IDs and flow-control state, while handles from the failed session terminate with that session. This preserves the project's current fail-fast recovery semantics without altering the wire protocol or its positioning.

## Change and expected effect

Added `native_mux_replacement_session_does_not_inherit_failed_stream_state` in `crates/espejismo-core/src/mux/native/tests.rs`. It writes on an old-session stream, drops its carrier, verifies the old stream reaches EOF, then opens a stream on a fresh session and verifies a full independent payload roundtrip. This guards against accidentally carrying stale stream state across reconnection. Expected performance change: none; this is correctness coverage only.

## Experiment and evidence

- `cargo test -p espejismo-core native_mux_replacement_session_does_not_inherit_failed_stream_state`: passed after aligning the stale-stream assertion with native mux read semantics (EOF is returned as `0`).
- `cargo test -p espejismo-core`: passed, 172 unit tests; includes the new session-boundary case and existing blocked-writer teardown regression. Integration/doc tests also completed successfully as part of the command.
- No throughput benchmark was run because this change does not alter the data path or claim a performance improvement.

## Conclusion

No transparent active-stream migration is implemented or claimed. The regression confirms that old stream state ends with the old carrier and that a new session can independently carry new streams, with no wire/protocol or performance change.
