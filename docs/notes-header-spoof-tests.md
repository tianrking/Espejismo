# HTTP/2 preface spoof boundary

## Findings and change

The optional HTTP fallback classifier previously treated the 16-byte prefix
`PRI * HTTP/2.0` as sufficient evidence of an HTTP/2 prior-knowledge request.
That accepts spoofed or truncated prefaces before the mandatory
`\r\n\r\nSM\r\n\r\n` suffix. The probe now reads up to the full 24-byte preface,
and the classifier selects fallback for HTTP/2 only when all 24 bytes match.
The existing narrow HTTP/1 method-prefix checks are unchanged.

The parser remains a small, socket-independent classifier, following the
explicit protocol-boundary testing approach referenced for sing-box and
shadowsocks-rust in `docs/research/REFERENCES.md`. This keeps fallback routing
separate from Espejismo's authenticated tunnel protocol and does not introduce
protocol camouflage.

## Expected benefit

Malformed and truncated HTTP/2 lookalikes no longer trigger the optional HTTP
fallback. This is a correctness/robustness change with no throughput claim.

## Verification

- `cargo test --offline -p espejismo-server fallback::tests` — 8 passed, 0
  failed. The new test checks every truncated length of the real 24-byte
  preface, the exact valid preface, and three lookalikes with missing or
  modified suffix bytes. Existing tests cover valid HTTP/1 methods, near-match
  methods, arbitrary binary prefixes and TLS records/ClientHello.
- `cargo test --offline -p espejismo-server` — 52 passed, 0 failed, 1 ignored
  (`requires loopback bind`, the existing SOCKS5 relay test).
- `git diff --check` — passed.

No regression was observed in the server package.
