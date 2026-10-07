# Auth bypass guard

## Scope and rationale

Review of `ingress/http_proxy.rs` and `ingress/socks5.rs` found that configured
local proxy authentication is enforced before a request reaches proxy handling:
HTTP checks `Proxy-Authorization` before sending CONNECT success, while SOCKS5
requires method `0x02` and validates credentials before parsing a request.
The missing gap was regression coverage for absent HTTP credentials and for a
SOCKS client offering both unauthenticated and authenticated methods. Added
tests for both cases and documented the rejection behavior in
`docs/deployment/AUTHENTICATION.md`. This only protects optional local proxy
listeners and does not change tunnel peer authentication or the project's
protocol positioning.

## Expected effect

No throughput change is expected; this is a regression-coverage change. The
tests guard against bypassing configured local credentials at both ingress
protocol negotiation points.

## Verification

`$HOME/.cargo/bin/cargo test --offline -p espejismo-core` passed: 200 unit
tests, 1 documented-config integration test, 6 HTTP proxy integration tests,
and 1 doctest. One existing test requiring loopback bind was ignored as
expected in this sandbox. The new SOCKS test confirms a mixed method offer
selects username/password and cannot proceed without those credentials. The new
HTTP integration test confirms absent credentials return 407 and never return
CONNECT success. No performance benchmark applies.
