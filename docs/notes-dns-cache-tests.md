# DNS cache boundary tests

## Findings and approach

`espejismo-core::dns` keeps successful platform resolver results for 60 seconds,
limits the process-wide cache to 256 authorities, and evicts the entry nearest
to expiry when full. This follows the small, bounded-cache approach used by
networking stacks such as sing-box and shadowsocks-rust: cache only resolved
addresses, keep lifetime and memory bounded, and avoid adding a new resolver or
changing the tunnel's operating model. The platform resolver does not expose
record TTLs, so the existing fixed short TTL remains unchanged.

The cache previously considered expiration only on lookup. On insertion, a
stale entry could still participate in choosing a live entry for eviction
(especially if entries had differing ages). Insertion now prunes expired
entries first. Added deterministic boundary tests cover pruning at the exact
expiration boundary, preserving live entries, replacing a key's result, and
refreshing its TTL. Existing coverage still checks cache hits immediately
before expiry, misses, the capacity ceiling, and duplicate-address handling.

## Expected effect

This is a correctness and bounded-state cleanup, not a throughput optimization.
It prevents stale cache state from displacing a still-valid mapping. Expected
lookup performance change is negligible; no benchmark claim is made.

## Verification

`cargo test -p espejismo-core dns::tests`: 16 passed, covering cache hit/miss,
TTL boundary, capacity and eviction, expiry pruning, key replacement/TTL refresh,
resolver retries/timeouts, numeric bypass, empty results, and address de-duplication.

`cargo test -p espejismo-core`: 256 unit tests passed, 1 loopback-bind test
ignored, plus 1 config example test, 10 HTTP proxy integration tests, and 1 doctest
passed. The ignored listener test requires loopback bind and is intended for
`cargo test -- --ignored` outside the sandbox. No performance benchmark was run
because this is a correctness-only cache maintenance change.
