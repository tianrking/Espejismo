# DNS TTL boundary tests

## Findings and change

`espejismo-core::dns` caches successful system-resolver results for a fixed 60 seconds because Tokio's system resolver does not expose DNS record TTLs. The cache already had a deterministic expiration-boundary test, but no coverage for zero or oversized lifetimes. The referenced projects in `docs/research/REFERENCES.md` offer no DNS TTL policy relevant to this fixed-lifetime system-resolver cache; this change keeps the existing small, bounded cache policy and does not alter protocol behavior.

Added a TTL-parameterized cache insertion helper for deterministic edge testing. Insertions cap requested TTL at the existing 60-second maximum before `Instant` arithmetic: zero TTL expires immediately, and even `Duration::MAX` neither overflows nor extends entries beyond the cache policy. Production inserts continue to use the unchanged 60-second TTL.

Expected improvement: no throughput change; prevents a panic from oversized TTL arithmetic if that path is exercised and defines edge behavior with regression tests.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core dns::tests --offline`: passed, 18 DNS tests; includes expiration boundary, zero TTL, oversized TTL, capacity pruning/eviction, resolver retries, timeout, and numeric endpoint paths. 0 failed, 0 ignored.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline`: the crate suite started (268 unit tests) and progressed through the output without an observed failure, but stopped making output progress for over 30 seconds; interrupted it. Full-crate completion remains unverified. The DNS-specific suite above completed successfully.
- No benchmark was run because this is a correctness and panic-safety change with no intended throughput effect.
