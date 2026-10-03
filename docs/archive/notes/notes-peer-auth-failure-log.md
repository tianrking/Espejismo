# Peer authentication failure logging

## Findings and approach

The remote handshake paths incremented failure metrics and recorded a runtime
error, but emitted no normal tracing event. This made routine journal/log
inspection miss failed peer authentication. The direct TCP and WebSocket/HTTP2
handlers now emit a `warn` event for rejected and timed-out authentication,
with the underlying protocol/I/O error for diagnosis. Events intentionally omit
peer addresses, user names, PSKs, handshake bytes, and traffic keys. The
handshake implementation returns generic authentication failures and does not
include the configured secret or candidate user name in those errors.

This is an operational visibility change only: it preserves the existing
authenticated-encrypted-chaos protocol and does not add an identity or protocol
fingerprint. It adds one warning per failed handshake; no throughput gain is
claimed.

## Verification

- Added a regression test for a malformed peer handshake, checking that the
  failure remains diagnosable while excluding configured secrets and the
  configured user name from the surfaced error.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core
  malformed_peer_handshake_error_does_not_disclose_configured_secrets` — passed
  (1 test).
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-server` — passed (17
  tests).
- No performance claim; authentication behavior and protocol bytes are
  unchanged.
