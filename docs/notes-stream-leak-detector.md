# Native mux unclosed stream teardown regression

## Findings and approach

The native mux owns per-stream entries and a dedicated frame-reader task inside
each physical session. The session teardown path closes flow state and aborts
the reader, but existing tests covered idle garbage collection and blocked
writers separately. They did not cover caller-held stream handles that remain
unclosed when the peer carrier disappears.

The yamux reference in `docs/research/REFERENCES.md` points to stream Drop and
session lifecycle handling as useful mux practices. This change follows that
lifecycle focus while retaining Espejismo's native authenticated transport and
does not change protocol framing or runtime behavior.

## Changes and expected benefit

- Added `native_mux_session_end_reclaims_unclosed_stream_state`, which keeps
  both stream handles alive without sending FIN/RST, drops one session, and
  checks that the opposite session task exits, both handles read EOF, and both
  reject writes with `BrokenPipe`.
- Documented that native mux stream handles are scoped to the physical session
  in `docs/ARCHITECTURE.md`.
- Expected benefit: catch regressions where unclosed handles or their session
  resources remain live after carrier loss. No runtime performance change is
  intended.

## Verification

- `cargo test -p espejismo-core mux::native::tests::native_mux_session_end_reclaims_unclosed_stream_state --offline`
  passed. This directly exercises unclosed handles on both endpoints and task
  termination after peer session loss.
- `cargo test -p espejismo-core --offline` passed: 179 unit tests, 1 config
  example integration test, 4 HTTP proxy integration tests, and 1 doc test.
- No throughput benchmark was run because this is a correctness regression
  test with no runtime change.
