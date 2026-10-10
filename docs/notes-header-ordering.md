# Header ordering hardening

## Findings and plan

The plain client handshake envelope is `[24-byte nonce][4-byte masked payload length][payload]`. The length is produced from a big-endian `u32` and then XOR-masked byte by byte. The bounded async parser consumes those fields in fixed order and rejects lengths outside the configured hello range before allocating/reading the payload. This follows the bounded, explicit framing approach documented for the reference projects (notably shadowsocks-rust's AEAD framing); no protocol or positioning change is needed.

The prior regression used one malformed rearrangement that mixed bytes across the nonce and length. Replace it with exhaustive coverage of all 23 non-identity permutations of the actual four length bytes, keeping nonce and payload unchanged. Expected gain: no throughput change; stronger assurance against accidental header byte-order/field-order tolerance and malformed-input regressions.

## Implementation and evidence

- Extended `handshake_rejects_truncated_and_reordered_client_packets` to try all 23 non-identity length-byte permutations. Every altered envelope must fail handshake parsing; the same test retains EOF cuts and the stealth truncation case.
- Updated `docs/PROTOCOL.md` to state that the masked length remains four ordered bytes decoded as big-endian `u32`, followed by the bounded payload.
- Targeted test: `cargo test -p espejismo-core handshake_rejects_truncated_and_reordered_client_packets` passed (1 test). It covered all 23 non-identity permutations, each required to be rejected, plus truncated envelope boundaries and a truncated stealth block.
- Full package test: `cargo test -p espejismo-core` passed (159 unit tests, 1 config example integration test, 4 HTTP proxy integration tests, and 1 doctest).
- Experiment conclusion: correctness hardening, not a performance change; no throughput claim applies. No protocol behavior changed for valid peers.
