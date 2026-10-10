# ACL boundary tests

## Findings and plan

`EgressPolicy` already checks deny rules before allow rules, leaves dimensions
unrestricted when their allowlists are empty, treats `*.example.com` as
including the apex, and rechecks direct resolved addresses. The boundary audit
found that IPv4-mapped IPv6 addresses took the IPv6 classification path and
were not checked against the embedded IPv4 ranges. This could let a private
IPv4 destination pass `deny_private_ips` when written as `::ffff:a.b.c.d`.

The fix classifies mapped addresses using their embedded IPv4 address. Added
unit cases cover mapped public/private literals and resolved addresses,
wildcard label boundaries and case folding, block-over-allow precedence,
empty allowlist semantics, and ports 0 and 65535. The deployment policy
reference now states the mapped-address behavior. No policy ordering or
project positioning changes. Expected benefit: close this representation
bypass; no measurable throughput change is expected.

## Validation

- `$HOME/.cargo/bin/cargo test -p espejismo-core egress::tests --offline`:
  passed, 10/10 ACL unit tests. Covers the mapped-address bypass regression,
  both literal and resolved address paths, wildcard matching boundaries,
  empty allowlists, block precedence, and port endpoints.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline`: passed, 224 unit
  tests, 1 ignored loopback-bind test, 1 config-example integration test, 8
  HTTP proxy integration tests, and 1 doctest. No failures.
- This is a correctness/security boundary change, not a performance change;
  no throughput benchmark applies. Conclusion: mapped private IPv4 targets
  are rejected in both ACL paths; no test regression observed.
