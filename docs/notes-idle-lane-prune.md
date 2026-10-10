# Idle client lane pruning

## Findings and approach

The client pool keeps a fixed set of lane slots and lazily establishes each
physical tunnel. Before this change, an established lane stayed connected until
its maximum connection age or a transport failure, even when the pool had been
quiet for a long time. The native mux already uses a 300 second idle lifetime.
The yamux guidance in `docs/research/REFERENCES.md` recommends explicit idle
session lifetimes; this change applies that resource-lifetime idea to excess
client physical lanes without changing the transport or protocol.

## Changes and expected benefit

- Before a requested stream is reserved, prune controls for lanes idle for at
  least 300 seconds, provided they have no active streams or pending opens.
- Keep at least the configured `min_connections` connected controls. The lane
  slots remain in the pool, so subsequent demand reconnects using the existing
  lazy connection and handshake path.
- Document the lane transition in `docs/ARCHITECTURE.md`.
- Expected benefit: after a quiet period followed by renewed demand, release
  excess physical sockets and their mux state while keeping the warm minimum.
  The trigger is the next stream request; this is not a periodic background
  reaper. Throughput is not expected to change during active traffic.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-client --offline` passed: all 41
  client tests. The pruning test covers timeout boundaries, future timestamps,
  active and pending lane protection, and the configured floor. A separate
  wake test verifies the pruned lane slot remains selectable and reserved for
  its existing lazy reconnect path.
- This is a correctness and resource-lifetime change, not a throughput
  optimization; no throughput benchmark was run and no throughput gain is
  claimed.
