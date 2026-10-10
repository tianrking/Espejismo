# TLS OCSP boundary tests

## Findings and plan

The HTTPS proxy uses rustls' default WebPKI certificate verifier. rustls passes a stapled OCSP response to `ServerCertVerifier`, but receipt is not OCSP signature or freshness validation. The existing transport test already covers absent, malformed, and large opaque staples reaching that callback. I added a strict test-only policy verifier to exercise rejection propagation for absent and malformed input. This follows rustls' verifier boundary and keeps the tunnel protocol and product positioning unchanged.

The verifier intentionally rejects without parsing OCSP. There is no signed expired-response fixture or OCSP validation dependency in this change, so this does not claim coverage of cryptographic signature validation or a real `nextUpdate` expiry. The plausible-DER-shaped case exercises the callback's rejection path, not actual expiry detection.

## Change and expected effect

Added an in-memory TLS 1.2 handshake test asserting that verifier rejection causes the client and server handshakes to fail for missing and malformed staples. The existing pass-through test continues to cover empty, 1-byte, 4 KiB, and 16 KiB responses. No production behavior or throughput changes; expected runtime impact is zero, with regression coverage for the verifier-to-handshake error path.

## Verification

- `cargo test --offline -p espejismo-server tls12_ocsp -- --nocapture`: passed, 2 OCSP tests; covers byte-for-byte callback delivery plus missing/malformed rejection propagation.
- `cargo test --offline -p espejismo-server`: passed, 66 passed, 0 failed, 1 ignored. The ignored test requires loopback bind; all in-memory TLS tests passed.
- No real expired signed OCSP response was tested; that remains a fixture/validator coverage gap.
