# Test Plan

The repository keeps automated checks focused on code and protocol correctness.
Release artifact construction lives in GitHub Actions.

## Required Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo check --manifest-path fuzz/Cargo.toml
```

CI runs the Rust checks on Linux, macOS, and Windows, and also builds release
binaries for native Linux amd64, macOS arm64, and Windows amd64. The release
workflow builds the complete published artifact matrix.

## Manual Smoke Path

Use two terminals and one config file:

```bash
cargo run --bin espejismo-remote -- --config ./espejismo.toml
cargo run --bin espejismo-local -- --config ./espejismo.toml
```

Then test the local proxies:

```bash
curl --socks5-hostname 127.0.0.1:6680 https://example.com/
curl -x http://127.0.0.1:6681 https://example.com/
```

For a remote handshake-only check:

```bash
cargo run --bin espejismo-local -- --config ./espejismo.toml --probe-server
```

## Unit Coverage

Current unit tests cover:

- Variable and stealth handshake completion.
- Multi-user handshake selection.
- Mux capability mismatch rejection.
- Tunnel request wire format.
- Frame encryption, stealth frames, and key update rotation.
- Puzzle verification and replay cache behavior.
- SOCKS5 UDP packet parsing.
- Native mux queues, flow control, GOAWAY, PING, and error paths.
- UDP underlay packet codec and reliability primitives.
- Config validation, including public admin listener token requirements.
- Egress policy host, port, and private IP rules.
- Admin authorization and bounded request parsing.

## Fuzz Targets

The `fuzz/` crate is outside the main workspace.

```bash
cargo install cargo-fuzz
cargo fuzz run socks5_udp_packet
cargo fuzz run config_toml
```

## Not Covered Yet

- Multi-hour soak tests.
- Cross-platform packet-loss simulation.
- Production UDP physical-underlay socket integration.
- Live physical tunnel migration.
- Browser/WASM transport behavior.
