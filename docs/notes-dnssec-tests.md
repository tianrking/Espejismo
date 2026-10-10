# DNSSEC boundary tests

## Findings and approach

The requested topic names DNSSEC validation, but the repository does not
implement DNSSEC validation. `crates/espejismo-core/src/dns.rs` delegates
hostname resolution to Tokio's platform resolver, which does not expose DNSSEC
validation status; there is no project-level DNSSEC policy to exercise. Adding a
DNSSEC client or claiming validation would change the resolver model and exceed
this boundary-test task. The references list recommends borrowing lightweight
Rust asynchronous I/O practices from shadowsocks-rust; this task keeps the
existing system-resolver design and adds no dependency or protocol.

As a bounded robustness fix, resolver results are now deduplicated while
preserving their original order. Duplicate addresses otherwise cause repeated
dial attempts during connection setup. This does not establish whether an
answer is DNSSEC-authenticated; that remains the responsibility (and current
limitation) of the platform resolver configuration. Expected benefit is
avoiding redundant attempts for duplicate results; no percentage is claimed
because this correctness change has no controlled performance benchmark.

## Verification

- `cargo test -p espejismo-core dns::tests`: 13 passed, 0 failed. The added
  `duplicate_resolver_addresses_are_returned_once_in_original_order` test
  covers repeated IPv4 and IPv6 results and stable ordering. Existing DNS
  tests cover empty results, cache expiration/capacity, retries, timeouts,
  malformed authorities, and numeric endpoint bypass.
- `cargo test -p espejismo-core`: 219 unit tests passed, 1 loopback test
  ignored by its existing sandbox annotation, 8 integration tests passed, and
  1 doctest passed; no failures.

Conclusion: duplicate resolver results no longer lead to duplicate dial
candidates. The DNSSEC validation feature itself is absent and is not claimed
by these tests.
