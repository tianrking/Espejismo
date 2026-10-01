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
- `native_mux_frame`: native mux frame header and length validator.

Each target has a checked-in starter corpus under `corpus/<target>/`. The cases are small,
hand-built valid and near-valid inputs intended to expose structural and boundary branches
before mutation. Run an individual target with `cargo fuzz run <target>`; libFuzzer will add
interesting inputs to that target's corpus during a run.
