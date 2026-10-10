# PROXY protocol v2 TLV framing

## Analysis and change

`parse_v2` already decoded the fixed IPv4/IPv6 address blocks and included the
declared TLV bytes in `consumed`, but treated that TLV region as unstructured.
Consequently it accepted a trailing one- or two-byte TLV header and TLVs whose
declared value extended beyond the v2 address block. The parser now walks TLV
framing after the fixed address block and rejects either malformed boundary;
TLV types and values remain opaque. Bytes after the declared header remain
available to the caller through the exact `consumed` offset.

`docs/research/REFERENCES.md` points to sing-box and shadowsocks-rust for
explicit, testable protocol handling. The change applies that narrow approach
to parser validation without enabling PROXY headers on a listener or changing
the project's positioning in `docs/POSITIONING.md`.

Expected benefit: malformed TLVs are rejected deterministically instead of
being accepted as valid client-address metadata. This is a correctness and
robustness change; no throughput improvement is claimed or expected.

## Verification

- `cargo test --offline -p espejismo-server proxy_protocol::tests`: passed, 5
  parser tests. Coverage includes v1 IPv4/IPv6, v2 IPv4/IPv6, valid TLVs,
  malformed TLV header/value lengths, payload boundary preservation, and
  malformed or truncated protocol headers.
- `cargo test --offline -p espejismo-server`: passed, 44 passed, 0 failed,
  1 ignored (`relay::tests::relays_tcp_through_two_socks5_hops`, requires
  loopback bind). The ignored test is unrelated to this socket-independent
  parser change.

No regression was observed. No performance benchmark was run because this is a
protocol correctness change and does not claim a performance gain.
