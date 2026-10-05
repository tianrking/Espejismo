# Testing Guide

This guide maps common changes to the checks that provide useful feedback. Run
commands from the repository root. The main Rust workspace contains the core,
client, server, and `tokio-yamux` crates; `fuzz/` is a separate Cargo project.
For the canonical required checks and current coverage limits, see the
[test plan](TEST_PLAN.md).

## Choose a check

| Change | Start with | Before sharing the change |
| --- | --- | --- |
| Rust code in one crate | `cargo test -p <package>` | Run the full workspace gate below |
| Protocol, crypto, config, or mux behavior | `cargo test -p espejismo-core` (or the affected package) | Add a regression test, then run the full workspace gate |
| Rust formatting or lint-sensitive code | `cargo fmt --all --check` and the Clippy command below | Run the full workspace gate |
| Fuzz target or parser boundary | `cargo check --manifest-path fuzz/Cargo.toml` | Run the relevant target for a bounded interval |
| End-to-end proxy behavior | Manual smoke path below | Record platform limits and any external service dependency |
| Throughput or transport tuning | Benchmark process below | Record comparable before/after results and spread |
| Documentation only | Review commands, links, and examples against their source docs | Run the documentation checks described in the change review |

`<package>` is the package name from the crate's `Cargo.toml`, for example
`espejismo-core`, `espejismo-client`, `espejismo-server`, or `tokio-yamux`.

## Fast local iteration

Run the smallest relevant check while editing, then run the full gate before
sharing Rust changes. Examples:

```bash
cargo test -p espejismo-core
cargo test -p espejismo-client
cargo test -p tokio-yamux
```

To run one test by name, append a substring filter:

```bash
cargo test -p espejismo-core handshake
```

Cargo compiles test targets as needed. `cargo test --workspace --all-targets`
also exercises benchmark targets that are configured as test targets; it does
not collect throughput measurements.

## Full Rust quality gate

These commands match the repository CI checks. Run them from the root:

```bash
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo check --manifest-path fuzz/Cargo.toml
```

CI runs formatting, workspace check, Clippy, and workspace tests on Linux,
macOS, and Windows. Fuzz target compilation runs on Linux. CI also builds
release binaries for its listed platform targets; consult
[`ci.yml`](../../.github/workflows/ci.yml) for the current matrix. A local pass
on one operating system does not establish behavior on the others.

## Fuzzing untrusted inputs

The fuzz crate is not a workspace member. Install `cargo-fuzz` once, then run
the target related to the parser or framing code you changed:

```bash
cargo install cargo-fuzz
cargo fuzz run socks5_udp_packet
cargo fuzz run config_toml
cargo fuzz run native_mux_frame
```

Each invocation continues until interrupted. For a bounded run, pass libFuzzer
options after `--`, for example `cargo fuzz run config_toml -- -max_total_time=60`.
Review newly generated corpus or crash files before including them in a change.
See the [fuzz README](../../fuzz/README.md) for target descriptions and starter
corpora.

## Manual client/server smoke check

Use a temporary test configuration and run the server and client in separate
terminals. The config must match the current schema and use test credentials;
the examples below assume it enables the local SOCKS5 and HTTP proxy listeners.

```bash
cargo run --bin espejismo-remote -- --config ./espejismo.toml
cargo run --bin espejismo-local -- --config ./espejismo.toml
```

Check both local proxy protocols from a third terminal:

```bash
curl --socks5-hostname 127.0.0.1:6680 https://example.com/
curl -x http://127.0.0.1:6681 https://example.com/
```

For a handshake-only probe, run the client with `--probe-server`. The addresses,
ports, and config path must match the test setup. These checks require a running
remote endpoint and network access; they are not part of `cargo test`.

## Throughput and performance work

Use [`BENCHMARKS.md`](BENCHMARKS.md) for prerequisites, harness variables,
output interpretation, and repeatability guidance. Use
[`TEST_PLAN.md`](TEST_PLAN.md) for a compact example invocation and coverage
overview. Keep the endpoint, workload, profile, parallelism, and environment
fixed across baseline and candidate runs. Report successful rounds, medians,
spread, and relevant environment; a noisy or failed run is not evidence of an
improvement.

The benchmark measures this tunnel and path. It does not change the product's
positioning or establish a universal protocol speed ranking; see
[`POSITIONING.md`](../POSITIONING.md).

## Reviewing documentation-only changes

Run every command added or changed in documentation only when its stated
prerequisites are available. Check relative links from the file containing
them, ensure examples agree with the canonical guide or implementation, and
state which checks were actually run. Do not report an unrun manual, fuzz, or
cross-platform check as passing.
