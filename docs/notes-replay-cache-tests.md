# Replay cache boundary tests

## Findings and change

`ReplayCache` stores timestamps in insertion order and prunes from the queue
front. A wall-clock rollback could previously append a lower timestamp behind a
higher one. When time advanced again, a still-live front entry could prevent a
later expired entry from being pruned, extending that identifier's replay
window. Inserted timestamps now use the later of `now` and the queue's last
timestamp, preserving the queue invariant without changing replay keys or
handshake behavior. Non-positive TTL values are treated as zero; a key remains
blocked at its insertion second and expires after time advances.

The approach keeps the bounded-time front-pruning design used by this cache;
the references list (including sing-box and shadowsocks-rust) is useful as a
reminder to keep replay state explicit and bounded, but provides no directly
applicable timestamp ordering policy. This change does not alter the protocol
or Espejismo's non-camouflage positioning.

## Expected impact

No throughput change is expected. Timestamp selection adds one queue-back
lookup on insert. It restores correct expiration after clock rollback and
prevents pathological retention of expired entries; correctness and bounded
cache lifetime are the intended gains.

## Validation

- `cargo test -p espejismo-core protocol::replay::tests`: passed, 10 tests.
- Covers duplicate detection, just-before/at/after TTL behavior, clock rollback,
  extreme timestamps, typed key separation, atomic handshake insertion, and
  zero/negative TTL semantics.
- `cargo test -p espejismo-core`: not completed; it stalled after the unit test
  output (the replay tests had passed) and was interrupted. The output showed
  the existing loopback-dependent listener test as ignored. No claim is made
  that the full crate suite passed.
