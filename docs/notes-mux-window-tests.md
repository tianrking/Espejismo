# Yamux Mux Window Boundary Tests

## Findings and scope

The vendored `tokio-yamux` implements per-stream send and receive windows.
Existing coverage exercises window-update batching, overflow rejection, and
writer wakeup races. The explicit zero-credit state was not directly asserted
through both directions: receive credit held at zero while data remains queued,
and a blocked writer remaining blocked on a zero-delta update.

The approach follows the window accounting and bounded flow-control practices
referenced for HashiCorp Yamux in `docs/research/REFERENCES.md`. This work adds
deterministic in-memory unit tests only; it does not change the wire protocol,
window sizing, or Espejismo's positioning.

## Changes and expected effect

- Added a receive-side test that starts at an exhausted window, verifies queued
  data prevents any credit update, then consumes the buffer and verifies a full
  window update restores the configured ceiling.
- Added a send-side test that verifies writes backpressure at zero, a zero-credit
  `WINDOW_UPDATE` leaves the writer blocked, and one byte of positive credit
  resumes exactly one byte of writing.
- No runtime behavior changed. Expected throughput impact is 0%; this is
  correctness coverage, not a performance optimization.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p tokio-yamux --lib` was run while
  iterating. The first run exposed invalid test construction state; after fixing
  both test streams to use a constructor-supported initial state, both new
  targeted tests passed.
- `$HOME/.cargo/bin/cargo test --offline -p tokio-yamux` passed: 52 unit tests,
  0 failed; doc tests passed. `one_way_bulk_transfer_exceeding_window` was
  ignored because it requires loopback bind, as required by the sandbox rule.
- The two new tests cover receive-window depletion/recovery and zero/positive
  send-credit transitions without socket creation. No throughput benchmark
  applies because runtime code and performance are unchanged.

## Conclusion

The zero-window boundary transitions now have deterministic regression
coverage. The available crate suite passed with the loopback integration test
ignored by design; no performance improvement is claimed.
