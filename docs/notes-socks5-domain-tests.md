# SOCKS5 domain boundary tests

## Findings and approach

SOCKS5 CONNECT encodes a domain as a one-byte byte length followed by UTF-8
bytes. The ingress parser retains valid non-empty UTF-8 verbatim in
`SocksTarget`; it does not perform local DNS or IDNA normalization. The server
relay resolves the resulting authority through the shared bounded resolver.
The existing resolver tests cover malformed-authority failure context and an
empty result from a resolver, while existing ingress tests cover empty,
NUL-containing, and invalid UTF-8 domains.

Following the narrow protocol-boundary style referenced for sing-box and
shadowsocks-rust in `docs/research/REFERENCES.md`, this change adds duplex
stream tests for the maximum representable 255-byte name, a Unicode IDN kept
byte-for-byte intact, and a request whose declared domain bytes are truncated.
No runtime behavior, dependencies, DNS placement, or project positioning is
changed. Expected benefit is regression coverage at SOCKS framing boundaries;
no performance gain is claimed.

## Verification

- `cargo test --offline -p espejismo-core ingress::socks5::tests`: 37 passed.
  The new cases cover the 255-byte maximum, UTF-8 IDN byte preservation, and
  truncated declared domain data. Existing cases cover empty, NUL, and
  malformed UTF-8 domain rejection.
- `cargo test --offline -p espejismo-core`: 300 unit tests passed, 1 existing
  loopback-bind test ignored; the documented-config integration test (1), HTTP
  proxy integration tests (10), and doctest (1) passed. Existing DNS coverage
  includes malformed-hostname resolution failure context and empty resolver
  results.
- No loopback tests were added and no performance change is claimed.

Conclusion: SOCKS5 domain framing boundaries and resolver failure reporting
have regression coverage without changing runtime behavior.
