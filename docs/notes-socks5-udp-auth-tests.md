# SOCKS5 UDP authentication and association boundary tests

## Findings and approach

The SOCKS5 ingress parser performs RFC 1929 negotiation on the TCP control
stream before reading the request command. UDP ASSOCIATE uses that same parser,
so a successful credential exchange on that control connection is the gate for
creating its UDP relay. Existing tests covered credential negotiation and UDP
ASSOCIATE independently, but did not assert their combined boundary.

Following the relay-boundary validation approach referenced by
[shadowsocks-rust](https://github.com/shadowsocks/shadowsocks-rust), tests now
exercise a valid authenticated UDP ASSOCIATE and ensure missing or incorrect
credentials cannot proceed to command parsing. In-memory duplex streams keep
these tests independent of local socket binds. Fragment-reassembly tests also
cover reusing the same peer for a fresh sequence after completion; the client
handler timeout test covers cancellation of an idle receive, and existing
expiry coverage confirms stale fragments do not contaminate later traffic.
These checks preserve the existing local proxy authentication model and do not
change the tunnel protocol or project positioning.

## Changes

- Add combined auth/UDP ASSOCIATE coverage for valid, missing, and invalid
  credentials on the control connection.
- Add regression coverage for sequential fragment-sequence reuse by one peer.
- Clarify in `docs/deployment/SOCKS5.md` that auth gates relay creation and
  completed reassembly state can be reused for a new sequence.

Expected improvement is correctness at authentication and reuse boundaries;
this change makes no throughput claim and has no expected performance impact.

## Validation

- `cargo test --offline -p espejismo-core ingress::socks5::tests`: 45 passed.
  Covers authenticated UDP ASSOCIATE acceptance/rejection, sequence reuse after
  completion and timeout, plus existing parser, fragment, endpoint, and size
  boundaries.
- `cargo test --offline -p espejismo-client udp_association_`: 2 passed.
  Covers successful receive within the idle window and cancellation on timeout.
- `cargo test --offline -p espejismo-core`: 328 unit tests, 1 config example,
  10 HTTP proxy integration tests, and 1 doctest passed; 1 test ignored.
- `cargo test --offline -p espejismo-client`: 56 passed.

No loopback bind was added. This is correctness-only work, so a throughput
benchmark is not applicable.
