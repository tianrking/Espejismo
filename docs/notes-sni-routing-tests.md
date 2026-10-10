# SNI and fallback routing boundary tests

## Findings and scope

The server does not select upstreams by SNI. Its optional HTTP fallback uses a
narrow prefix classifier before tunnel authentication; TLS ClientHello traffic
(including SNI-bearing ClientHello) stays on the authenticated tunnel path.
This matches `docs/POSITIONING.md`: no TLS camouflage or new multi-upstream
routing behavior is introduced. The routing principle used here is explicit
protocol-boundary classification, consistent with the modular protocol
handling used by sing-box and Xray-core (see `docs/research/REFERENCES.md`).

This change expands the executable boundary coverage: all ten currently
recognized HTTP method/preface prefixes must match, while incomplete, lowercase,
and near-match method prefixes must not select fallback. Existing tests continue
to cover TLS record prefixes and an SNI-bearing ClientHello, including
truncations. The classifier itself and runtime behavior are unchanged.

## Expected effect

This correctness-only change prevents accidental routing regressions at the
HTTP/tunnel boundary. It has no throughput claim or expected performance
change.

## Verification

`cargo test --offline -p espejismo-server` passed: 49 passed, 0 failed, 1
ignored. `detects_common_http_methods` exercises every currently recognized
HTTP method and HTTP/2 prior-knowledge prefix. The new
`rejects_incomplete_or_near_match_http_methods` test covers incomplete,
lowercase, and extended method names. Existing
`tls_client_hello_with_sni_is_not_routed_as_http` checks the complete SNI
ClientHello and all truncated prefixes; `tls_record_prefixes_never_select_http_fallback`
covers partial TLS record types. These are in-memory classifier tests and do
not require loopback sockets; the ignored package test is the pre-existing
SOCKS5 loopback-bind test.

Conclusion: all targeted HTTP, TLS, and SNI routing boundaries pass with no
classifier behavior change.
