# Log sanitization

## Scope and approach

`Debug` formatting is a common path into diagnostic logs. Upstream proxy parsing
stores credentials separately in `EgressProxy`, while its original URI can also
remain in `EgressPolicy`; local SOCKS/HTTP proxy authentication is stored in
`ProxyAuth`. These values previously derived `Debug`, which exposed passwords
(and local usernames) if formatted. Their custom implementations now replace
credential values with `<redacted>`; egress policy keeps its nonsecret rule
fields visible while redacting configured proxy URIs. Updated the Rust style
guide to require this safeguard for credential-bearing types.

This is a narrow hardening change: it does not alter authentication, proxy
configuration, or normal protocol logs. Expected security benefit is removal of
these credential disclosure paths from accidental `Debug` logging; there is no
runtime performance impact beyond formatting the same small structs.

## Verification

- `cargo test -p espejismo-core --offline` — passed: 171 unit tests, 5 integration/doc tests, and 1 doctest.
- Added focused tests asserting `Debug` output for local proxy credentials,
  parsed upstream credentials, and configured upstream proxy URIs does not
  contain usernames or passwords, while parsed proxies show `<redacted>`.
- No throughput benchmark applies; this is formatting-only hardening.
