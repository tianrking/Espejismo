# UDP relay fragment boundary tests

## Findings and approach

SOCKS5 UDP fragmentation is reassembled at the client's SOCKS ingress in
`SocksUdpReassembler`; the UDP relay itself still sends one reassembled request
through the existing tunnel. `docs/notes-udp-fragment-reassembly.md` records the
five-second expiry, ordered sequence handling, source/target isolation, and
16-bit tunnel payload cap. The protocol reference list points to
shadowsocks-rust's bounded UDP relay approach; this task keeps that bounded,
explicit protocol behavior and does not change the tunnel format or project
positioning.

The existing unit tests covered ordinary two-fragment assembly, gaps, malformed
input, peer/target changes, and oversize input, but not expiry or the highest
legal SOCKS5 fragment sequence. Add deterministic in-memory tests for those
boundaries. The expiry test moves the private deadline into the past instead of
waiting five seconds. The sequence test assembles all 127 fragments, including
the final marker on sequence 127.

Expected result: regression coverage for expiry reset and maximum sequence
assembly; no performance gain is claimed because this is correctness-only.

## Verification

- `cargo test --offline -p espejismo-core ingress::socks5::tests`: 29 passed,
  0 failed. Exercises ordered reassembly, final marker handling, gap/malformed
  reset, peer/target isolation, payload cap, expiry, and sequence 127.
- `cargo test --offline -p espejismo-core`: 281 unit tests passed, 1 ignored
  (`requires loopback bind`), 10 HTTP proxy integration tests, 1 config example
  integration test, and 1 doctest passed. No failures.
- `cargo fmt --all -- --check` reports pre-existing formatting differences in
  unrelated client, core, and server files; the command made no changes.
- This correctness-only change has no throughput claim, so no benchmark applies.
