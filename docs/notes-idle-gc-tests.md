# Native mux idle session garbage collection tests

## Findings and approach

The native mux session loop starts its idle timer only while its active stream
map is empty. Once the timeout fires it sends `GOAWAY` and enters the bounded
drain period. Receiving `FIN` or `RST` removes a stream from the map. Existing
coverage checked that an initially empty session exits, but did not prove that
an active session survives past the idle duration or that concurrent stream
closure allows both peers to be reclaimed.

The `yamux` reference in `docs/research/REFERENCES.md` recommends explicit
session idle and drain timeouts. This work tests the native mux's existing
policy without changing timeout behavior, framing, or Espejismo's transport
positioning.

## Changes and expected benefit

- Added an eight-stream regression test. It keeps both session tasks alive
  beyond the configured idle timeout, closes all streams concurrently, and
  verifies both sessions finish within a bounded deadline.
- Clarified the native mux idle transition in `docs/ARCHITECTURE.md`.
- Expected benefit: detect premature reclamation while streams are active and
  failure to reclaim sessions after the last stream closes. No runtime or
  performance change is intended.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core mux::native::tests --offline`
  passed: all 16 native mux tests, including the new concurrent idle-GC test.
  The new test covers survival beyond the idle timeout with eight active
  streams and reclamation at both peers after they close.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline` passed: 152 unit
  tests, 1 config example integration test, 4 HTTP proxy integration tests,
  and 1 doc test.
- No throughput benchmark was run because this is a correctness-only test
  change with no intended runtime behavior change.
