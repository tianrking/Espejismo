# TLS fallback boundary tests

## Findings and plan

The server checks optional HTTP fallback before authenticating the Espejismo
tunnel. `looks_like_http_probe` is intentionally a narrow prefix classifier for
known HTTP methods; TLS ClientHello bytes (including an SNI-bearing example)
already have a regression test proving they are not routed to fallback. The
positioning and protocol references explicitly reject TLS camouflage or
protocol impersonation. The relevant transferable testing principle from
projects such as sing-box and Xray-core is to keep transport routing decisions
at explicit protocol boundaries, rather than infer a protocol from vague
similarity.

This change documents that boundary beside the classifier and adds a table-like
test over TLS handshake, alert, change-cipher-spec, and application-data record
types, including every partial record-header prefix. It does not add TLS
negotiation or change fallback routing. Expected outcome: regression coverage
for false-positive HTTP fallback selection at protocol boundaries, with no
throughput impact expected for this correctness-only change.

## Validation

`cargo test --offline -p espejismo-server` passed: 46 passed, 0 failed, 1
ignored (the existing ignored SOCKS5 loopback test). The new
`tls_record_prefixes_never_select_http_fallback` test covers four TLS record
content types across all prefix lengths; the existing
`tls_client_hello_with_sni_is_not_routed_as_http` test covers a realistic SNI
ClientHello and truncations. HTTP positive cases and generic binary negatives
also passed. The new tests use in-memory byte slices and do not bind loopback
sockets. `rustfmt` was applied to the changed Rust file; package-wide formatting
check reports unrelated existing formatting diffs in `http_chain.rs`,
`main.rs`, and `proxy_protocol.rs`.

Conclusion: the optional HTTP probe classifier has regression coverage against
TLS record prefixes and retains its explicit-method-only boundary. No
performance claim applies to this correctness change.
