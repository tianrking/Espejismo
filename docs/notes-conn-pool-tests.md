# Connection pool behavior tests

## Findings and approach

The client constructs a fixed set of interactive and bulk lanes, bounded by
`max_connections`, and retires an established physical connection when its
age reaches `max_connection_age_secs`. Stream acquisition serializes lane
selection with the pending-open reservation; that reservation must be visible
to the next concurrent selector. Existing tests covered lane scoring but not
pool sizing, age boundaries, or concurrent reservations.

The reference guidance in `docs/research/REFERENCES.md` points to explicit,
bounded resource accounting in mature transport implementations. This change
keeps the existing fixed TCP/yamux lane pool and adds deterministic tests for
its current invariants. It does not change the project's transport or product
positioning.

## Changes and expected benefit

- Extracted lane layout construction and connection-age comparison into small
  helpers used by the manager.
- Kept lane scoring and the pending reservation inside one serialized
  selection path. Concurrent opens therefore include earlier pending opens in
  subsequent lane scores.
- Added tests for minimum/maximum lane counts, expiry just before and exactly
  at the configured age, and 32 concurrent acquisitions distributing evenly
  across two equally loaded bulk lanes.
- Expected benefit: catch regressions in pool hard-cap enforcement, idle
  session rotation boundaries, and duplicate selection under concurrent load.
  No throughput or latency gain is claimed; runtime behavior is otherwise
  unchanged.

## Verification

Ran `cargo test -p espejismo-client tunnel::tests --offline` on de. Result: all
11 tunnel tests passed, including the new pool-layout, connection-age, and
concurrent-acquisition tests. The concurrency test verifies all 32 pending
opens are recorded and split 16/16 across the two lanes. No benchmark was run
because this is a correctness and testability change with no intended
performance effect.
