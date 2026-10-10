# Concurrent close handling

## Findings and approach

`tokio-yamux` keeps stream and session state in the session task. Multiple
application tasks can independently own stream handles or cloned `Control`
handles, so their close events are serialized through the existing event and
command channels. The Yamux reference in `docs/research/REFERENCES.md` points
to explicit lifecycle handling; this change applies that principle without
changing the wire protocol or Espejismo's positioning.

Repeated session shutdown handling previously sent another GoAway and replaced
the close timeout each time. A burst of concurrent `Control::close` requests
could therefore create redundant frames and extend the drain deadline. Local
GoAway is now idempotent: later close requests preserve the first frame and
deadline. Concurrent stream shutdowns remain independent and each emits its
own FIN followed by a close event when its handle is dropped.

Expected benefit: bounded shutdown timing under concurrent close requests and
no duplicate session GoAway frames. This is a correctness change; no throughput
gain is expected.

## Verification

- `cargo test -p tokio-yamux repeated_session_close_requests_send_one_go_away`
  passed. It makes 16 sequential shutdown requests at the session boundary and
  confirms exactly one frame is written.
- `cargo test -p tokio-yamux concurrent_stream_shutdowns_emit_one_fin_and_close_each_stream`
  passed. `join_all` drives 64 stream shutdown futures together and checks that
  all 64 FIN frames and all 64 close events are emitted.
- `$HOME/.cargo/bin/cargo test -p tokio-yamux --lib` passed: 40 tests, 0
  failures.
- `$HOME/.cargo/bin/cargo test -p tokio-yamux` passed all 40 library tests,
  then the existing `window_update_deadlock` integration test failed before
  exercising mux behavior because its local socket bind returned
  `PermissionDenied (Operation not permitted)` in this sandbox.
- `$HOME/.cargo/bin/cargo fmt --all -- --check` reports pre-existing formatting
  differences in unrelated workspace files. The two edited Rust files pass
  their targeted `rustfmt --check`.
- No throughput benchmark was run because this is a correctness change with no
  intended performance impact.
