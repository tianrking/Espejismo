# HAProxy PROXY protocol parser tests

The server had no PROXY protocol parser or tests. Added a small, socket-independent
parser module for the HAProxy PROXY v1 TCP4/TCP6 lines and v2 PROXY command with
IPv4/IPv6 address blocks. It returns source/destination socket addresses and the
exact header length so callers can preserve application bytes following the
preamble. V2 TLV bytes are included in the consumed length; TLV interpretation
and `LOCAL`/`UNKNOWN` address reporting are not part of this narrow parser.

The reference guide's sing-box and shadowsocks-rust pointers reinforce keeping
protocol handling explicit and independently testable. This change does not
enable PROXY headers on the listener, change ingress behavior, or alter the
project's non-camouflage positioning; a trusted-proxy configuration and listener
integration remain a separate design decision.

Expected benefit: deterministic coverage of valid IPv4/IPv6 inputs, payload
boundary handling, and rejection of malformed, oversized, unsupported, or
truncated frames without requiring sockets. No performance improvement is
claimed.

Verification: `cargo test --offline -p espejismo-server proxy_protocol::tests`
passed all 4 focused parser tests. `cargo test --offline -p espejismo-server`
passed 40 tests, with 1 existing loopback-dependent test ignored. Tests exercise
v1 IPv4/IPv6 and payload offset, v1 malformed/oversized input, v2 IPv4/IPv6 and
TLV length accounting, and v2 signature/family/version/truncation failures.
No regression was observed in the server package.
