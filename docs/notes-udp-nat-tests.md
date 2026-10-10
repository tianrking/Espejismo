# UDP peer mapping boundary tests

## Findings and approach

SOCKS5 UDP ASSOCIATE binds one UDP socket per control association and returns each relay response to the source `SocketAddr` that sent that datagram. Fragment reassembly is also association-local: its state includes the source address, destination, payload, and a five-second expiry. This follows the per-association ownership used by UDP relay implementations such as [shadowsocks-rust](https://github.com/shadowsocks/shadowsocks-rust), without introducing a global mapping table or changing Espejismo's tunnel model.

The boundary tests exercise the existing mapping ownership rules without binding sockets: concurrent associations with the same peer cannot combine fragments, changing a peer port invalidates an in-flight sequence, and a reused peer can start a new sequence after expiry. The five-second fragment timeout is now a named constant so its policy is explicit.

Expected benefit: no throughput change; stronger regression coverage for cross-association isolation and stale mapping state after peer-port reuse. No SOCKS relay protocol or externally visible behavior changes.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core socks_udp_reassembler --offline`: 10 passed. This filter included expiry, source mismatch, payload limit, and ordered fragment cases.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline`: 302 unit tests passed, 1 pre-existing ignored; integration tests 11 passed; doc tests 1 passed. Includes the new concurrent-association and expired-peer-reuse cases.
- `cargo fmt --all -- --check` reports formatting differences across unrelated existing files, so it was not used to reformat the workspace. `rustfmt --edition 2021 crates/espejismo-core/src/ingress/socks5.rs` formatted the changed source file.
- No loopback sockets or performance benchmark are needed: this is an in-memory correctness test and changes no performance path.
