# Yamux Window Boundary Tests

## Findings and scope

`tokio-yamux` already batches receive credit until the calculated increase is
at least half the configured maximum, forces an update when stream flags must
be sent, and uses `checked_add` to reject send-credit overflow. Existing tests
covered receive-window overrun and writer wakeups, but did not pin the exact
batching threshold or assert that a rejected overflowing update leaves credit
unchanged.

The upstream reference list points to HashiCorp Yamux for window management.
This change keeps the existing Yamux flow-control mechanism and adds regression
coverage without changing the protocol, configuration, or Espejismo's
positioning. The deployment flow-control documentation now states the exact
half-window threshold and overflow behavior.

## Changes and expected effect

- Add a unit test proving an increase just below half the receive ceiling is
  withheld, while an increase exactly at half is advertised and restores the
  receive window to its ceiling.
- Add a unit test proving an overflowing `WINDOW_UPDATE` is rejected without
  modifying the prior send credit.
- Clarify the existing overflow guard in code and document both behaviors.

These are correctness tests; no throughput change is expected or claimed.

## Verification

- `cargo test -p tokio-yamux`: all 30 library unit tests passed, including both
  new boundary tests and existing receive-overrun, writer-wakeup, and window
  sizing tests. The integration test `one_way_bulk_transfer_exceeding_window`
  could not start because the sandbox denied `TcpListener::bind` with
  `PermissionDenied` (`Operation not permitted`). It therefore did not provide
  a passing full-crate test run in this environment.
- Performance benchmark: not applicable; the implementation does not alter the
  flow-control algorithm or claim a performance improvement.

## Conclusion

The added deterministic unit coverage passes. The socket-dependent integration
regression remains unverified here because local loopback socket binding is
blocked by the environment; rerun `cargo test -p tokio-yamux` where loopback
sockets are permitted before treating the full crate suite as green.
