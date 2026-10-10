# IP and host blacklist boundary tests

## Findings and plan

`EgressPolicy::validate_authority` evaluates `block_hosts` before allow rules.
Exact host rules are case-insensitive; `*.example.com` includes the apex and
subdomains while requiring a DNS label boundary. Existing tests checked
wildcard boundaries for the allowlist and one block-over-allow case, but did
not independently pin the blocklist's exact and wildcard edge behavior.

Added unit coverage for wildcard block rules against the apex, nested
subdomains, case variation, lookalike prefixes, and appended suffixes. Added
exact-rule cases for case-insensitivity and ensuring that subdomains and
longer suffix names do not match. Updated the egress reference to document
these blocklist matching boundaries. This is a correctness-only change; the
expected throughput change is 0% because policy behavior is unchanged and
only tests and explanatory documentation were added.

## Validation

- `$HOME/.cargo/bin/cargo test -p espejismo-core egress::tests --offline`:
  passed, 12/12 egress tests. The two new tests cover wildcard block matching
  and exact block matching at case and label/suffix boundaries.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline`: passed, 226 unit
  tests, 1 ignored loopback-bind test, 1 config-example integration test, 8
  HTTP proxy integration tests, and 1 doctest. No failures.
- No loopback bind was introduced; the ignored test is pre-existing.

Conclusion: host blacklist matching rejects only the configured exact name or
the wildcard's apex and true subdomains; no test regression observed.
