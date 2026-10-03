# Connection limit enforcement

## Findings and approach

The remote listener already creates a global semaphore from
`shared.max_physical_connections` and attempts a non-blocking permit for each
accepted socket. When no permit is available it drops the socket and emits a
debug log entry; the permit remains held for the full `handle_peer` lifetime.
This matches the bounded-admission approach used in async server designs such
as sing-box's bounded resource controls while preserving Espejismo's explicit,
authenticated TCP tunnel behavior.

The missing coverage was a regression test proving the configured capacity
limits live handlers and becomes available again when a connection ends. The
implementation keeps the existing policy and factors capacity calculation and
permit acquisition into small functions used directly by the accept loop and
test. Expected improvement: no throughput change; deterministic enforcement
coverage prevents accidental unbounded physical connection admission.

## Verification

`cargo test -p espejismo-server` passed: 11 tests passed, 0 failed. The new
regression test verifies capacity two admits exactly two simultaneous permits,
rejects the third, and admits a new connection after release. No throughput
change was intended or measured; this is a correctness and regression test.
