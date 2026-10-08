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
