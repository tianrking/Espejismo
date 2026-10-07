# OCSP stapling boundary tests

## Findings and scope

The HTTPS proxy client uses rustls' normal certificate verifier. Its verifier callback receives the server's OCSP staple, while rustls' built-in WebPKI verifier currently logs non-empty responses as unvalidated; receiving a staple must not be described as validating it. The existing test-only verifier discarded the callback argument, leaving this boundary untested.

Reference checked: rustls 0.23's `ServerCertVerifier` API and `CertifiedKey::ocsp` support transport testing without adding dependencies. This follows the references document's preference for focused protocol-state tests. It does not change Espejismo's protocol or claim OCSP validation support.

## Change and expected effect

Added a TLS 1.2 loopback handshake test that staples zero bytes, a one-byte malformed payload, and a 4096-byte opaque payload. A capture verifier asserts rustls forwards each byte sequence unchanged, covering absent, tiny, and larger response boundaries. Added a test-only verifier that delegates handshake signature checks to the existing verifier helper.

Expected effect: no runtime or throughput change; improved regression coverage for OCSP response delivery to the verifier callback. Certificate status policy remains the verifier's responsibility.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-server --offline`: passed; 42 passed, 0 failed, 1 ignored. This includes the new callback-boundary test and the existing HTTPS proxy handshake, session resumption, timeout, and untrusted-certificate tests.
- `$HOME/.cargo/bin/cargo fmt --all -- --check`: reports formatting differences across unrelated pre-existing files; formatted only `crates/espejismo-server/src/http_chain.rs` to avoid unrelated changes.
