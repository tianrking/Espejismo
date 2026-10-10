# TLS SNI routing boundary tests

## Findings and approach

The server does not implement SNI-based backend selection. Inbound connection
classification recognizes only its explicit HTTP fallback prefixes; all other
prefixes continue to authenticated tunnel handling. This preserves the project
positioning in `docs/POSITIONING.md` and avoids adding TLS impersonation or a
multi-backend routing model.

Following the modular protocol-boundary approach noted for sing-box and
Xray-core in `docs/research/REFERENCES.md`, the classifier now returns an
explicit internal route decision. The SNI cases remain deliberately outside
that decision: missing SNI, wildcard-shaped input, and distinct hostnames all
resolve to the authenticated tunnel path. HTTP fallback still requires a
recognized HTTP prefix.

Expected effect: clearer routing boundary and regression protection for future
changes, with no performance claim or expected throughput change.

## Verification

`$HOME/.cargo/bin/cargo test --offline -p espejismo-server` passed: 64 passed,
0 failed, 1 ignored (the existing SOCKS5 test requiring loopback bind).
`fallback::tests::sni_values_never_select_a_fallback_backend` covers missing,
wildcard-shaped, and two distinct hostname inputs, plus the positive HTTP
fallback boundary. Existing tests cover a complete SNI-bearing ClientHello,
truncated ClientHello prefixes, empty/malformed/oversized SNI structures, TLS
record prefixes, and HTTP method/preface boundaries. These are in-memory tests
and require no loopback socket.

Conclusion: the tested inputs preserve the intended fixed routing behavior;
there is no performance change to measure and no observed correctness
regression.
