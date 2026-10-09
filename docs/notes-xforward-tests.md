# X-Forwarded header boundary tests

## Findings and scope

The HTTP proxy does not parse or trust `X-Forwarded-*` fields. For absolute-form
HTTP requests, `rewrite_absolute_request` forwards ordinary headers verbatim;
the only removed field is `Proxy-Authorization`. This keeps forwarding metadata
as opaque client input and avoids implying that Espejismo authenticates a
forwarding chain. The behavior matches the project's minimal proxy role and
does not change its tunnel protocol or positioning. No external proxy
implementation is needed for this local parser behavior.

## Change and expected result

Added a duplex-stream regression test covering a comma-separated
`X-Forwarded-For` chain, a second field with different casing, tab/space around
an `X-Forwarded-Proto` value, and `X-Forwarded-Host` with a port. The test
asserts both duplicate preservation and byte-level field value preservation
after absolute-form request rewriting. A nearby code comment records that
forwarding headers stay opaque and are not trusted for access control.

Expected improvement: no runtime performance change; this is correctness
coverage intended to prevent accidental normalization, merging, or loss of
forwarded identity metadata.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core ingress::http_proxy::tests` — 8 passed, 0 failed.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core` — 259 unit tests passed, 1 ignored (requires loopback bind); 1 config example integration test passed; 10 HTTP proxy integration tests passed; 1 doctest passed.
- The added test uses `tokio::io::duplex` and requires no loopback socket.

Conclusion: forwarded header chains, duplicate field lines, casing, and surrounding value whitespace survive rewriting; no correctness regressions observed in the core crate suite.
