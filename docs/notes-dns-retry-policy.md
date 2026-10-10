# DNS retry policy

## Findings and approach

`espejismo_core::dns::resolve_socket_addrs` used Tokio's platform resolver with
one attempt and a 10 second timeout. Transient resolver errors therefore
immediately failed connection setup. `docs/research/REFERENCES.md` emphasizes
bounded resource behavior and learning implementation techniques without
changing the product's resolver model; it has no DNS retry recipe to adopt.
The implementation continues to use the operating system resolver and adds no
DoH/DoT behavior or resolver configuration.

The resolver now makes at most three attempts, sleeping 100 ms and then 250 ms
between failures. The full retry sequence remains inside the existing 10
second timeout. Success ends retries and populates the existing short-lived
cache; failed lookups remain uncached. This provides up to two recovery chances
for a transient failure, while limiting added delay to 350 ms on immediate
failures. No latency improvement is claimed; the expected benefit is improved
connection setup success during short-lived resolver errors.

## Verification

- `cargo test -p espejismo-core dns::tests`: 12 passed. New injected-resolver
  tests verify retry through two failures to success, early stop at success,
  exactly three attempts on exhaustion, last-failure context, and the existing
  overall timeout. Existing cache, empty-result, malformed-authority, and
  numeric IPv4/IPv6 bypass coverage also passed.
- `cargo test -p espejismo-core`: 203 unit tests passed, 1 loopback test was
  ignored, 9 integration tests passed, and 1 doctest passed. The ignored test
  requires loopback bind; run `cargo test -p espejismo-core -- --ignored`
  outside the sandbox as documented by the test.

Conclusion: the bounded retry policy passes the core correctness suite. The
maximum attempt count is enforced and total async wait remains capped at the
existing 10 second deadline. This is a correctness/availability change, so no
throughput benchmark or percentage claim applies.
