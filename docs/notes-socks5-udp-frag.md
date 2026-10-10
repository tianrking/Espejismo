# SOCKS5 UDP FRAG boundary coverage

## Findings and approach

The SOCKS5 ingress parser (`crates/espejismo-core/src/ingress/socks5.rs`) and
the SOCKS5 proxy response decoder (`crates/espejismo-server/src/socks5_chain.rs`)
already reject nonzero FRAG values. Coverage only exercised FRAG=1, leaving the
remaining byte values and malformed address-header boundaries unpinned.

The protocol reference is RFC 1928 section 7: FRAG=0 denotes an unfragmented
datagram and nonzero values carry fragment sequence/final-fragment information.
This project does not reassemble SOCKS5 fragments, so unsupported values must be
rejected consistently. The reference inventory is `docs/research/REFERENCES.md`;
no upstream algorithm change is needed for this parser-boundary task, and no
dependency or proxy behavior is changed. The changes add exhaustive
FRAG-byte rejection checks and truncate-at-every-byte tests for IPv4, IPv6, and
domain address headers, including empty payload acceptance at the complete
header boundary.

## Expected impact

No throughput change is expected: this is regression coverage for the existing
parser behavior, not a performance change. It makes the unsupported-fragment
contract explicit for every possible FRAG byte and helps catch future panics or
accidental acceptance at address-header boundaries. Positioning remains
unchanged: no protocol camouflage, new service, or operational dependency.

## Verification

Ran `$HOME/.cargo/bin/cargo test --offline -p espejismo-core` and
`$HOME/.cargo/bin/cargo test --offline -p espejismo-server`: core passed 183
unit tests plus its 1 config-doc test, 4 HTTP proxy integration tests, and 1
doctest; server passed 36 tests with 1 existing loopback-bind test ignored.
The new coverage exercised every nonzero FRAG byte (1–255) in both codecs and
every truncated prefix for IPv4, IPv6, and domain headers. No regressions were
observed. This is correctness coverage, so throughput benchmarking is not
applicable.
