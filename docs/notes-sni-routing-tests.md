# SNI routing test notes

## Findings and scope

The server currently routes only recognized HTTP request prefixes to the
optional HTTP fallback (`fallback.rs`); it does not parse SNI or select
upstreams by hostname. TLS ClientHello traffic therefore remains on the
authenticated tunnel path. This matches the project's positioning: this change
adds a regression boundary test and does not add TLS camouflage or a new
multi-upstream routing feature.

## Change and expected effect

Added a synthetic TLS ClientHello carrying an `example.com` SNI extension to
the fallback classifier tests. The test checks both the complete record and
every truncated prefix, ensuring none can be mistaken for an HTTP method. This
does not claim a throughput gain; it prevents accidental cross-protocol
fallback routing and documents the current SNI behavior in executable form.

## Verification

`cargo test -p espejismo-server` passed: 43 passed, 0 failed, 1 ignored
(the ignored loopback-bind test requires an environment capability). This ran
the complete server crate unit-test suite, including the new complete and
truncated ClientHello/SNI classifier boundary cases.

`cargo fmt --check` was attempted but reports pre-existing formatting diffs in
unrelated files throughout the workspace. The changed Rust file was formatted
directly with `rustfmt --edition 2024`.
