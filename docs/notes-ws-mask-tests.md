# WebSocket mask boundary tests

## Findings and approach

`crates/espejismo-core/src/underlay.rs` already enforced client/server mask-bit roles and decoded the RFC 6455 four-byte mask key, but the read and write paths duplicated the XOR loop. Existing WebSocket tests focused on payload-size encoding and did not pin the mask-key cycle at the 4-byte boundary or directly verify truncated mask-key handling.

The Xray-core underlay abstraction is a useful reference from `docs/research/REFERENCES.md`: the WebSocket carrier remains a transport adapter beneath the authenticated tunnel. This change preserves that separation and protocol behavior. It extracts the shared XOR operation and covers key-cycle lengths around 4-byte boundaries, reversibility, role-specific mask expectations, and EOF during the mask key. The tests use in-memory Tokio duplex streams and do not require loopback sockets.

Expected benefit: stronger regression coverage and one shared implementation for mask/unmask byte indexing; no throughput change is expected or claimed.

## Changes

- Added `apply_websocket_mask` and used it in both frame encoding and decoding.
- Added deterministic tests for payload lengths 0, 1, 3, 4, 5, 8, and 9 with a fixed key.
- Added frame-reader tests for valid client/server mask-bit roles, role mismatches, and a truncated mask key.

## Validation

- `$HOME/.cargo/bin/cargo test -p espejismo-core underlay::tests::websocket --offline`: passed, 14 tests (including mask cycle, role, truncation, frame length, ping/close, and underlay behavior); 239 unrelated unit tests filtered out.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline`: passed; 252 unit tests passed, 1 existing loopback test ignored, 1 config-doc integration test passed, 10 HTTP-proxy integration tests passed, and 1 doctest passed.
- `$HOME/.cargo/bin/cargo fmt --all -- --check`: reports existing formatting differences across unrelated files. The changed Rust file was formatted directly with `rustfmt --edition 2024` to avoid modifying unrelated code.

## Follow-up: decoded mask vector and peer-role check

The prior role test used empty payloads for valid frames, so it proved mask-bit
acceptance but did not exercise key application in the frame reader. The role
predicate is now isolated as `websocket_mask_matches_peer_role`, and a reader
test feeds the RFC 6455 masked `Hello` vector (`37 fa 21 3d` key) and checks the
decoded payload. The same test verifies a server rejects an unmasked client
data frame. This extends coverage without changing the wire format or adding
socket-based tests.

No throughput optimization was made, so no performance delta is claimed. The
work targets correctness and has no expected throughput impact.

Validation for this follow-up:

- `cargo test --offline -p espejismo-core underlay::tests::websocket`: passed,
  20 tests; includes mask-cycle edges, role mismatches, truncated keys, RFC
  masked payload decoding, and explicit unmasked-client rejection.
- `cargo test --offline -p espejismo-core`: passed, 317 unit tests, 1 existing
  loopback test ignored, 1 config-doc integration test, 10 HTTP proxy integration
  tests, and 1 doctest.
