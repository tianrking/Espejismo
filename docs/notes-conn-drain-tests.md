# Connection drain boundary tests

## Findings and approach

The server's shutdown path already stops dispatching accepted peers and waits
for active peer tasks up to `PEER_SHUTDOWN_GRACE`; `drain_peer_tasks` aborts
remaining tasks once the deadline expires. Listener tasks forward sockets over
an mpsc channel. Keeping its receiver alive during the drain would allow those
tasks to continue queuing newly accepted sockets, so shutdown now drops the
receiver before waiting. A failed send makes each listener task exit.

This follows the bounded-drain model documented by Yamux in
`docs/research/REFERENCES.md`: finish existing work within a fixed window and
then close remaining work. It does not add session migration or change the
project's transport identity.

## Expected effect

No throughput change is expected. During shutdown, newly accepted sockets are
dropped instead of queued for handling, and peer shutdown remains bounded by
the configured grace period. This avoids admitting work that cannot be
processed after shutdown begins.

## Verification

- `cargo test -p espejismo-server restart_drain_tests` — passed (3 tests):
  active peers complete during grace, overdue peers are aborted, and listener
  forwarding fails after shutdown drops the admission receiver.
- `cargo test -p espejismo-server` — passed: 65 passed, 1 ignored loopback
  test, 0 failed. The ignored test requires loopback bind, which this sandbox
  disallows; it is unrelated to the channel-based drain coverage.
- The tests cover completion within grace, forced abort at timeout, and failure
  to forward a newly accepted connection after admission closes. These tests
  use task/channel primitives and do not bind loopback sockets.
