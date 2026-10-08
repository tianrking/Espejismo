# HAProxy PROXY protocol v1 boundary tests

## Findings and change

The server has a socket-independent parser for HAProxy PROXY v1/v2. Its v1
length check previously counted only bytes before CRLF and allowed a line longer
than the protocol's 107-byte maximum. It also reported an overlong unterminated
line as merely incomplete. The [HAProxy PROXY protocol specification](https://github.com/haproxy/haproxy/blob/master/doc/proxy-protocol.txt)
defines the 107-byte maximum including CRLF. This round corrects the accounting
and rejects an unterminated input once it can no longer fit within that bound.
ASCII is checked explicitly as required by v1.

Added a regression test for a 107-byte line (which proceeds to field validation),
a 108-byte line, an oversized unterminated line, and preservation of bytes after
CRLF. Existing IPv4/IPv6, malformed-field and port-range tests remain in place.

## Approach and expected benefit

Following the references' emphasis on explicit, independently testable protocol
parsing (sing-box and shadowsocks-rust), the fix stays inside the pure parser and
uses byte-level boundary inputs; it needs no listener/socket and adds no runtime
dependency. It closes acceptance of oversized v1 preambles and prevents callers
from treating irrecoverably oversized partial data as a header still in flight.
This is a correctness/robustness change; no throughput improvement is claimed.
It does not enable PROXY headers on the listener or change the project's
non-camouflage positioning.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-server proxy_protocol::tests` — 6 passed, 0 failed. Covers v1 maximum size, over-limit complete and incomplete lines, IPv4/IPv6 parsing, payload offset, malformed fields, plus existing v2 parser cases.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-server` — 48 passed, 0 failed, 1 ignored (`requires loopback bind`). The ignored test is the existing SOCKS5 relay loopback test, unrelated to this change.
- `git diff --check` — passed.

No regression was observed in the server package.
