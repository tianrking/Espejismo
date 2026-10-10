# Early data buffering tests

## Findings and approach

The tunnel does not negotiate TLS-style 0-RTT application data. The relevant
boundary is the authenticated handshake followed by encrypted framing: bytes
sent immediately after a successful handshake can already be queued on the
underlay before the server starts `spawn_frame_transport`. Handshake parsing
uses bounded `read_exact` calls for the handshake envelope, so it should leave
subsequent frame bytes available to the frame reader. This follows the general
stream discipline used by Rust async I/O and the buffering discipline in
shadowsocks-rust; no protocol or product positioning change is needed.

The change adds a transport regression test that completes both handshakes,
writes a valid encrypted Data frame from the client before creating the server
frame pump, and verifies that the queued payload is released intact. Expected
gain: improved regression coverage for handshake-to-frame handoff; there is no
runtime or throughput change.

## Experiment

- `$HOME/.cargo/bin/cargo test -p espejismo-core frame_sent_immediately_after_handshake_is_released_to_transport` — passed (1 targeted test).
- `$HOME/.cargo/bin/cargo test -p espejismo-core` — passed (180 unit tests, 5 integration tests, 1 doc test).
- The regression exercises the client Data-frame send immediately after authentication, queued on the underlay before the server frame pump starts, then verifies byte-for-byte release. No throughput claim applies because the code path and runtime behavior are unchanged.

Conclusion: the handoff preserves early queued application data; no test regressions.
