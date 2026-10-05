# Stream and mux error-kind tests

## Scope and expected result

`tokio-yamux::StreamHandle` converts internal stream states and malformed input
into `std::io::ErrorKind` values consumed by callers. The existing tests covered
reset reads and receive-window violations, but did not pin the write-after-close
classification or the `peek` malformed-frame path. This change adds focused
regression tests for those paths without changing runtime behavior or the mux
protocol. Expected benefit is improved classification stability; there is no
throughput claim for a test-only correctness change.

## Verification

`$HOME/.cargo/bin/cargo test -p tokio-yamux --lib` passed: 27 tests, including
reset/local-close writes as `BrokenPipe`, receive-window violations through
`read` and `peek` as `InvalidData`, and the `GoAway` response. The full
`cargo test -p tokio-yamux` reran those 27 tests successfully, but its
`window_update_deadlock` integration test could not open its local socket pair
in this restricted environment (`PermissionDenied`, OS error 1). This failure
occurs before exercising mux behavior; it is an environment limitation rather
than a regression. No throughput measurement applies to this test-only change.

The workspace `$HOME/.cargo/bin/cargo test` likewise completed all workspace
unit and documentation tests successfully (including all 27 `tokio-yamux` unit
tests), then stopped at the same socket-permission failure in
`window_update_deadlock`. The full correctness gate therefore remains
environment-blocked; its integration test must be rerun where local socket
creation is permitted.
