# SNI probe classifier boundary tests

## Findings and plan

The optional HTTP fallback is selected by `looks_like_http_probe`, which only
recognizes known HTTP method prefixes and the complete HTTP/2 prior-knowledge
preface. Existing coverage included an ordinary SNI-bearing ClientHello and
its truncations, but did not name the empty, oversized, and malformed SNI
cases called out by this task. The transferable principle in
`docs/research/REFERENCES.md` is explicit protocol-boundary handling, as used
by modular proxy projects such as sing-box and Xray-core. This task keeps the
classifier narrow and leaves the project's no-camouflage positioning intact.

Added a classifier regression test for an empty/dangling SNI extension, TLS
and handshake lengths that exceed received input, and an SNI extension whose
declared length exceeds the bytes present. Each complete sample and each
prefix, including the empty prefix, must remain outside HTTP fallback. The
fixtures target routing classification; they do not attempt to validate TLS.
Expected benefit is preventing accidental HTTP fallback selection for these
TLS-shaped edge inputs. This correctness-only change has no throughput claim
or expected performance impact.

## Verification

`$HOME/.cargo/bin/cargo test --offline -p espejismo-server` passed: 55 passed,
0 failed, 1 ignored. The ignored test is the existing SOCKS5 test requiring
loopback bind. `fallback::tests::empty_oversized_and_malformed_sni_client_hellos_are_not_http`
covers the specified empty, oversized, and malformed cases across every byte
prefix. Existing tests additionally cover a normal SNI ClientHello and its
truncations, TLS record prefixes, positive HTTP method/preface cases, and
near-match HTTP negatives. All additions are in-memory tests and require no
socket. Formatting was applied with rustfmt using the workspace Rust 2021
edition.

Conclusion: SNI-related TLS-shaped inputs remain excluded from optional HTTP
fallback across the tested boundaries; no runtime classifier behavior changed.
