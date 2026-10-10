# OCSP stapling boundary tests

## Findings and scope

The HTTPS proxy client uses rustls' normal certificate verifier. Its verifier callback receives the server's OCSP staple, while rustls' built-in WebPKI verifier currently logs non-empty responses as unvalidated; receiving a staple must not be described as validating it. The existing test-only verifier discarded the callback argument, leaving this boundary untested.

Reference checked: rustls 0.23's `ServerCertVerifier` API and `CertifiedKey::ocsp` support transport testing without adding dependencies. This follows the references document's preference for focused protocol-state tests. It does not change Espejismo's protocol or claim OCSP validation support.

## Change and expected effect

Added a TLS 1.2 in-memory duplex handshake test that staples zero bytes, a one-byte malformed payload, a 4096-byte opaque payload, and a 16 KiB opaque payload. A capture verifier asserts rustls forwards each byte sequence unchanged, covering empty, minimal, typical, and near-record-sized response boundaries (including record fragmentation). Added a test-only verifier that delegates handshake signature checks to the existing verifier helper.

Expected effect: no runtime or throughput change; improved regression coverage for OCSP response delivery to the verifier callback, including a larger fragmented response. Certificate status policy remains the verifier's responsibility. The test uses in-memory streams and requires no loopback bind.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-server --offline`: passed; 53 passed, 0 failed, 1 ignored. `tls12_ocsp_staple_bytes_reach_certificate_verifier_unchanged` confirms empty, 1-byte, 4 KiB, and 16 KiB values are delivered byte-for-byte; the test-only verifier also delegates both TLS signature verification methods. The ignored loopback relay test is unrelated to this change.
- `$HOME/.cargo/bin/cargo fmt --all -- --check`: reports formatting differences across unrelated pre-existing files; formatted only `crates/espejismo-server/src/http_chain.rs` to avoid unrelated changes.
