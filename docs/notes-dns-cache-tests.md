# DNS cache tests

## Findings and approach

The shared `resolve_socket_addrs` helper had no application cache, so repeated
connection setup for the same hostname always entered Tokio's platform
resolver. Tokio's `lookup_host` does not expose DNS record TTLs. This change
adds a bounded process-local cache with a 60 second fixed TTL and a 256-entry
limit. Numeric socket addresses still bypass the cache and resolver; failed or
empty lookups are not cached. Expiration uses monotonic `Instant` time, and an
entry is expired at the exact TTL boundary.

The implementation keeps the existing resolver API and 10 second timeout.
The bounded state and small helper approach follow the lightweight Rust async
IO style called out for shadowsocks-rust in `docs/research/REFERENCES.md`;
there is no new dependency and no change to transport or project positioning.
The expected gain is that repeated lookups for a cached authority avoid a
system resolver round trip entirely. No percentage is claimed: this correctness
task does not include a controlled DNS benchmark, and system resolver latency
varies by host. The fixed TTL is a deliberate limitation because the current
platform resolver API does not provide authoritative record TTLs.

## Verification

- `cargo test -p espejismo-core dns::tests`: 7 passed. Tests cover a cache hit
  before expiry, expiration exactly at the TTL boundary and entry removal, a
  cold miss, the existing timeout/error/empty-result cases, and numeric IPv4
  and IPv6 bypass behavior.
- `cargo test -p espejismo-core`: 164 unit tests, 5 integration tests, and 1
  doctest passed; no failures.

Conclusion: repeated successful hostname lookups can be served from the
bounded cache for up to 60 seconds; TTL boundary behavior and existing DNS
resolution paths pass the core suite.
