# Handshake malformed-input coverage

## Research and plan

`docs/research/REFERENCES.md` points to shadowsocks-rust for readable Rust AEAD
framing and to mature transport projects for state-machine discipline. Applied
here as bounded, explicit parser-boundary tests; no protocol or positioning
change is appropriate for this task. `docs/PROTOCOL.md` already requires
incomplete handshakes to receive no application data.

The plain client hello is read as a 24-byte nonce, 4-byte masked length, then a
bounded payload. Stealth mode reads one configured-size block. Add deterministic
EOF truncations at these boundaries and inside payload, a reordered plain
header, and a truncated stealth block. These tests verify rejection only; no
throughput gain is expected, and there is no runtime behavior change.

## Implementation and evidence

Added `handshake_rejects_truncated_and_reordered_client_packets` in
`crates/espejismo-core/src/crypto/mod.rs`. It covers short nonce, partial nonce,
short length, empty and truncated payload, reordered header fields, and one-byte
short stealth block. Updated `docs/PROTOCOL.md` with the parser ordering and EOF
behavior.

Validation: `$HOME/.cargo/bin/cargo test --offline -p espejismo-core
handshake_rejects_truncated_and_reordered_client_packets` passed (1 test; all
listed malformed shapes rejected). `$HOME/.cargo/bin/cargo test --offline -p
espejismo-core` passed: 153 unit tests, 1 config documentation integration test,
4 HTTP proxy integration tests, and 1 doctest; no failures. This is robustness
coverage, so no throughput improvement is claimed or expected.
