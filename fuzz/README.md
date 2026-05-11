# Espejismo Fuzz Targets

These targets exercise untrusted input parsers without joining the main Cargo workspace.

Run with:

```bash
cargo install cargo-fuzz
cargo fuzz run socks5_udp_packet
cargo fuzz run config_toml
```

Current targets:

- `socks5_udp_packet`: SOCKS5 UDP packet parser.
- `config_toml`: TOML configuration parser and validators.
