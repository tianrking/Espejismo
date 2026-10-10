# Shutdown race tests

## Findings and approach

The existing native mux tests covered session drain timeout waking a writer
blocked on send-window credit, and separate tests covered transport EOF and
session lifecycle. They did not exercise a pending read and blocked write on
the same stream while session shutdown reclaimed that stream. `docs/research/REFERENCES.md`
identifies explicit lifecycle handling as a useful yamux practice; this test
checks the native mux's existing teardown behavior without changing its wire
protocol or Espejismo's positioning in `docs/POSITIONING.md`.

## Changes and expected benefit

- Added a regression test that splits one stream into concurrent read and write
  tasks, fills the four-byte send window, then starts GOAWAY drain timeout.
- The test requires the pending read to return EOF and the blocked write to
  return `BrokenPipe` after teardown.
- Expected benefit: deterministic coverage against stranded read or write tasks
  during shutdown; no throughput change is expected from this test-only code.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core native_mux_goaway_timeout_wakes_concurrent_stream_read_and_write`
  passed (1 test). It covers concurrent pending read and flow-control-blocked
  write during GOAWAY timeout teardown.
- `$HOME/.cargo/bin/cargo test -p espejismo-core` passed: 174 unit tests, 1
  config example integration test, 4 HTTP proxy integration tests, and 1 doc
  test.
- This is a correctness-only change; no throughput benchmark was run.
