# UDP reassembly boundary tests

## Findings and approach

SOCKS5 UDP fragment reassembly lives in `SocksUdpReassembler` at the client
ingress. It keeps one ordered sequence, expires incomplete data after five
seconds, and caps the joined payload at the tunnel's 16-bit UDP payload limit.
The shadowsocks-rust UDP relay reference in `docs/research/REFERENCES.md`
supports the same bounded-buffer approach; these tests preserve Espejismo's
existing SOCKS ingress and tunnel protocol.

Existing tests covered timeout expiry and over-limit rejection. The missing
boundaries were a repeated sequence number (which must invalidate the sequence)
and a payload exactly equal to the wire cap (which must remain accepted). Both
are deterministic, in-memory tests and do not create sockets.

## Change and expected result

- Added a duplicate-fragment regression test. It verifies the duplicate resets
  queued bytes and a later fragment from that stale sequence cannot complete;
  a fresh final first fragment can start and complete normally.
- Added an exact-limit assembly test for 65,535 payload bytes, complementing the
  existing test that rejects 65,536 bytes.
- No runtime code or protocol behavior changed. Expected performance impact is
  none; the additional coverage protects ordered sequence handling and the
  memory boundary against regressions.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core
  ingress::socks5::tests` — passed: 43 passed, 0 failed. The reassembly tests
  cover ordered assembly, timeout expiry, duplicate/gap handling, source and
  target isolation, the 127-fragment ceiling, and exact/over-limit payloads.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core` — passed: 315 unit
  tests, 1 ignored (`requires loopback bind`), 10 HTTP proxy integration tests,
  1 config example integration test, and 1 doctest; zero failures.
- No benchmark applies: this is correctness coverage only and changes no runtime
  behavior.
