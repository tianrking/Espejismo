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

For the live HK2 to RK throughput tuning record, see
[`THROUGHPUT_TUNING_HK2_RK.md`](./THROUGHPUT_TUNING_HK2_RK.md).

## Throughput Benchmark Harness

Use `scripts/bench-throughput.sh` on a Linux test client to run direct and
proxied download/upload checks with the same payloads:

```bash
espejismo-bench-http --listen 0.0.0.0:18082

ESPEJISMO_PROXY_URL=http://127.0.0.1:16681 \
ESPEJISMO_DIRECT_DOWNLOAD_URL=http://203.0.113.10:18082/256m.bin \
ESPEJISMO_PROXY_DOWNLOAD_URL=http://127.0.0.1:18082/256m.bin \
ESPEJISMO_DIRECT_UPLOAD_URL=http://203.0.113.10:18082/upload \
ESPEJISMO_PROXY_UPLOAD_URL=http://127.0.0.1:18082/upload \
ESPEJISMO_ADMIN_URL=http://127.0.0.1:9090/status \
ESPEJISMO_ADMIN_TOKEN=change-me-admin-token \
ESPEJISMO_PARALLEL=4 \
ESPEJISMO_ROUNDS=5 \
scripts/bench-throughput.sh
```

The script writes raw curl output, `results.jsonl`, `summary.md`, and optional
admin snapshots into `bench-results/<run-id>/`. It also writes
`environment.md` and `log-risk.md`. The log-risk check scans the recent local
log tail and fails by default if it sees giant lines or mux frame-body dumps,
because verbose transport logs can make proxy throughput look falsely slow.
Set `ESPEJISMO_LOCAL_LOG_FILE` when the local process writes somewhere other
than `/tmp/espejismo-local-bench.log`; set `ESPEJISMO_ALLOW_VERBOSE_LOGS=1`
only for intentional debugging runs whose numbers will not be used for tuning.

`summary.md` includes per-run results, median/mean/min/max/stddev, and
proxy/direct efficiency for matching tests. Use it when comparing mux,
frame-size, pacing, or lane-pool changes so raw link variation and Espejismo
overhead are recorded in the same run.

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
- Admin authorization, bounded request parsing, and lane metric rendering.

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
