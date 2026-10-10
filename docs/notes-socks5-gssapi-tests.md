# SOCKS5 GSSAPI method boundary tests

## Findings and approach

The SOCKS5 ingress method negotiation in `crates/espejismo-core/src/ingress/socks5.rs`
implements only no-auth (`0x00`) and RFC 1929 username/password (`0x02`). GSSAPI
(`0x01`) has no subnegotiation implementation. Tests should lock down both this
unsupported-method response and the policy-controlled fallback behavior: with
no local credentials configured the listener may choose no-auth if offered;
with credentials configured it must choose username/password and never silently
downgrade to no-auth or pretend to accept GSSAPI.

The repository references guide review of established proxy implementations;
the applicable lesson from shadowsocks-rust's relay boundary testing is to test
the negotiation boundary directly. This task does not add GSSAPI or a dependency,
preserving Espejismo's small operational model and existing local-auth design.

## Changes and expected impact

- Add in-memory duplex tests for GSSAPI-only rejection, explicit no-auth policy
  selection when GSSAPI and no-auth are offered, and username/password selection
  when credentials are configured alongside GSSAPI.
- Document the supported methods and the policy-driven no-auth selection in the
  SOCKS5 deployment guide.

Expected impact is correctness and regression protection only; no performance
change is intended.

## Validation

- `cargo test --offline -p espejismo-core ingress::socks5::tests`: 48 passed,
  covering GSSAPI-only rejection, configured no-auth selection when offered,
  credential-required preference for RFC 1929, and all existing SOCKS5 parser,
  UDP association, and fragmentation boundaries.
- `cargo test --offline -p espejismo-core`: 338 unit tests passed, 1 ignored;
  the config example integration test, 10 HTTP proxy integration tests, and
  1 doctest passed.

No loopback bind was added. This is correctness-only work, so a throughput
benchmark is not applicable. There was no failure or performance claim.
