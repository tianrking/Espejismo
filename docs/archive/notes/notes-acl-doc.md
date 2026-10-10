# Egress ACL documentation

## Findings and plan

The existing deployment guide listed the `[remote.egress]` fields but did not
explain the empty-allowlist behavior, case handling, wildcard apex matching, or
the full evaluation order. This made it harder to predict which rule wins and
to write a restrictive policy safely.

The implementation in `crates/espejismo-core/src/egress.rs` establishes that
host matching is case-insensitive; `*.example.com` matches both the apex and
subdomains; host and port block rules are checked before their corresponding
allow rules; and empty allowlists do not restrict that dimension. Direct
resolved addresses are checked again for private/special IPs and ports. Host
rules match the originally requested hostname, not the resolved address.

Updated `docs/deployment/EGRESS.md` with those semantics, the request check
order, a restrictive HTTPS allowlist example, and expected allowed/rejected
targets. This is documentation-only and preserves the single-purpose tunnel
and minimal operations model. Expected performance, latency, and resource-use
change: 0%.

## Validation and experiment

- Compared the text and example against `EgressPolicy::validate_authority`,
  `validate_resolved_addr`, and `host_matches` in
  `crates/espejismo-core/src/egress.rs`.
- Manually traced the example: `example.com:443` and
  `api.example.com:443` pass; `admin.example.com:443` is denied by the host
  blocklist; port 80 is denied by the port allowlist; and `example.net:443` is
  denied by the host allowlist.
- No benchmark or Rust tests apply because no executable source changed.
  Conclusion: the described outcomes match the implementation; runtime
  behavior is unchanged and there is no runtime regression. Expected
  performance impact: 0%.
