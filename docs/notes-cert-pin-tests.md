# HTTPS proxy certificate boundary tests

## Findings and plan

There is no certificate pinning in Espejismo. The HTTPS egress proxy client
uses rustls with bundled WebPKI roots and derives the TLS server name from the
proxy endpoint. Existing coverage rejects a self-signed certificate, but did
not exercise malformed endpoint names. Keep the existing trust-chain policy
and add a focused in-memory test proving an invalid name is rejected before a
TLS ClientHello is sent. This preserves the project's certificate ownership
boundary and does not add tunnel TLS, custom trust, or certificate pins.

Expected effect: catch regressions that could accept or send a TLS handshake
with an invalid proxy identity. No runtime or throughput change is expected.

## Verification

- `cargo test --offline -p espejismo-server https_proxy_rejects_invalid_tls_server_name_before_handshake`: passed (1 passed, 54 filtered out). The test checks rejection of a whitespace-containing TLS server name and verifies the peer receives no bytes.
- `cargo test --offline -p espejismo-server`: passed (54 passed, 0 failed, 1 ignored). Existing untrusted-certificate rejection, TLS session behavior, OCSP boundary, timeout, and all other package tests passed. The ignored SOCKS relay test explicitly requires loopback bind.

Conclusion: malformed HTTPS proxy TLS identities fail before handshake traffic; all non-ignored server package tests pass. No performance claim applies to this correctness change.
