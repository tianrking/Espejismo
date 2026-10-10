# PROXY protocol parser boundary tests

## Scope and expected effect

The server's v1/v2 parsers already checked their fixed address sizes and v1's
107-byte line limit. The tests did not systematically exercise every truncated
prefix, nor isolate v2's unsupported command/family and a declared block much
larger than the supplied input. Add regression coverage at those parser
boundaries without changing accepted protocol behavior. This is a correctness
and robustness change; expected throughput improvement is 0% because parsing
logic is unchanged.

The parsers are socket independent, matching the bounded-input approach used
by established proxy implementations: validate the preamble before handing the
remaining stream to the application. This preserves Espejismo's existing
trusted-front-proxy role and does not alter its transport or positioning.

## Changes

- Exercise every strict prefix of valid v1 and v2 headers and require an error.
- Check v2's unsupported LOCAL command and address family errors explicitly.
- Check that a `u16::MAX` declared v2 block with only an address block present
  is rejected as incomplete.
- Existing tests continue to cover v1 line length, malformed addresses and
  ports, v2 short address blocks, TLV truncation, and payload preservation.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-server proxy_protocol`:
  8 passed, 0 failed (50 unrelated tests filtered out).
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-server`:
  57 passed, 0 failed, 1 ignored. The ignored test requires loopback bind;
  the suite reports no failures outside that sandbox-restricted test.
- No performance benchmark applies; no performance claim is made.
