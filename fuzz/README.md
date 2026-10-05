# Espejismo Fuzz Targets

These targets exercise untrusted input parsers without joining the main Cargo workspace.

Run with:

```bash
cargo install cargo-fuzz
cargo fuzz run socks5_udp_packet
cargo fuzz run config_toml
cargo fuzz run native_mux_frame -- -runs=100000
```

Current targets:

- `socks5_udp_packet`: SOCKS5 UDP packet parser.
- `config_toml`: TOML configuration parser and validators.
- `native_mux_frame`: native mux frame header and length validator.

The native mux target checks both panic freedom and parser invariants: complete
frames stay valid when trailing bytes are present, and selected header/payload
truncation boundaries stay incomplete. For each valid frame it checks all nine
incomplete header lengths and the final payload truncation boundary. Its
regression counterpart lives in the core frame unit tests.

Each target has a checked-in starter corpus under `corpus/<target>/`. The cases are small,
hand-built valid and near-valid inputs intended to expose structural and boundary branches
before mutation. Run an individual target with `cargo fuzz run <target>`; libFuzzer will add
interesting inputs to that target's corpus during a run.
