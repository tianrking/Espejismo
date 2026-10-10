# DNS Rebinding Address Boundary Tests

## Findings and approach

The egress path already validates resolved TCP/UDP destination addresses when
`remote.egress.deny_private_ips` is enabled, which is the boundary needed to
resist a hostname changing from a public answer to a private one. The classifier
covered RFC1918, loopback, link-local, broadcast, IPv4 documentation, and
selected IPv6 local addresses, but missed shared-use, benchmarking, IPv6
multicast, and documentation space. This change extends that existing opt-in
policy and tests the resolved-address validator directly; it does not change
the default egress policy or tunnel positioning.

Added a table-driven regression test covering loopback, intranet/private,
link-local metadata, shared-use, documentation and reserved IPv4, benchmarking,
multicast, IPv6 local and documentation ranges, plus public IPv4/IPv6 controls.
Updated the Egress Policy reference to state the expanded range coverage.

## Expected effect

No throughput impact is expected: classification remains a small set of
address-family checks performed at destination validation. The security effect
is rejecting those otherwise-missed special addresses when the configured
`deny_private_ips` policy is active.

## Experiment

Ran `$HOME/.cargo/bin/cargo test -p espejismo-core deny_private_ips_rejects_dns_rebinding_address_boundaries --offline`:
1 passed. The targeted test exercised resolved-address rejection for loopback,
private/intranet, link-local, shared-use, documentation/reserved, benchmarking,
multicast, IPv6 local, and IPv6 documentation values, with public-address
controls retained.

Ran `$HOME/.cargo/bin/cargo test -p espejismo-core --offline`: 322 unit tests
passed, 1 existing loopback-bind test was ignored, the 1 config example test
passed, all 10 HTTP proxy integration tests passed, and 1 doctest passed. No
failures. The ignored test is unrelated to this change and requires loopback
bind, which the sandbox blocks.
