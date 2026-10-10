# SOCKS5h remote resolution boundaries

## Findings and approach

SOCKS5 `DOMAINNAME` requests are preserved as a hostname in `SocksTarget`.
The server relay later forms the destination authority and calls the shared
`resolve_socket_addrs` helper, so hostname resolution happens at the remote
egress (the behavior applications request with `socks5h://`). The shared
resolver already provides a bounded timeout and bounded positive cache. The
SOCKS5 ingress previously accepted a zero-length UTF-8 domain and returned a
success reply before the remote resolver could reject it; malformed UTF-8 also
failed without a SOCKS reply.

Following the protocol-focused parsing and bounded async I/O approaches listed
for sing-box and shadowsocks-rust in `docs/research/REFERENCES.md`, this change
keeps the existing hostname forwarding and resolver design. It rejects empty,
non-UTF-8, and NUL-containing domain names at ingress with SOCKS reply `0x08`.
No DNS lookup is performed locally, no dependencies are added, and project
positioning is unchanged. Expected benefit is clearer protocol behavior and
failure at the requesting boundary; no performance gain is claimed.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core ingress::socks5::tests`:
  17 passed. New tests verify a hostname is retained verbatim for remote
  resolution and that empty, malformed UTF-8, and NUL-containing names all
  fail with reply `0x08`.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core`: 191 unit tests
  passed, 1 loopback-bind test ignored by its existing annotation; all 1 config
  integration test, 5 HTTP proxy integration tests, and 1 doctest passed.
- `rustfmt --edition 2021 --check crates/espejismo-core/src/ingress/socks5.rs`:
  passed. Workspace `cargo fmt --all -- --check` reports pre-existing format
  differences in unrelated files, so no workspace-wide formatting was applied.

Conclusion: remote hostname forwarding and the malformed-domain reply boundary
are covered by deterministic local tests. This correctness change has no
performance claim.
