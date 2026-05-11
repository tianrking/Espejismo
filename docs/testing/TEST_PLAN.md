# Espejismo Test Plan

## Goals

The test suite must prove that Espejismo works as a real proxy, not merely as a
set of compiling protocol primitives.

The core end-to-end invariant is:

1. A client application sends a request to `espejismo-local`.
2. `espejismo-local` opens a mux stream over the encrypted physical tunnel.
3. `espejismo-remote` receives the logical stream and opens the requested
   outbound TCP connection.
4. The outbound service receives the exact request identity.
5. The service response travels back through `espejismo-remote`,
   `espejismo-local`, and finally to the client application.

## Automated Checks

Run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets
cargo check --manifest-path fuzz/Cargo.toml --bins
./scripts/e2e_smoke.sh
REQUESTS=200 CONCURRENCY=32 ./scripts/stress_smoke.sh
MUX_MODE=native ./scripts/e2e_smoke.sh
REQUESTS=128 CONCURRENCY=32 ./scripts/benchmark_mux.sh
./scripts/package-release.sh
```

On Windows, use:

```powershell
.\scripts\e2e_smoke.ps1
.\scripts\stress_smoke.ps1 -Requests 200 -Concurrency 16
.\scripts\package-release.ps1
```

## End-to-End Probe

`scripts/e2e_smoke.sh` starts four processes:

- `scripts/probe_http_server.py`: deterministic HTTP fixture.
- `espejismo-remote`: remote tunnel endpoint loaded from base64 TOML.
- `espejismo-local`: local proxy endpoint loaded from file TOML.
- `curl`: client probe.

The probe sends a unique token through these paths:

- Authenticated SOCKS5 GET
- Sequential SOCKS5 GET requests with `max_streams = 2`, which proves the
  remote stream limit is concurrent rather than lifetime-cumulative.
- Authenticated SOCKS5 POST with body echo
- Authenticated SOCKS5 UDP ASSOCIATE datagram relay
- Authenticated HTTP proxy absolute-form GET
- Authenticated HTTP CONNECT tunnel
- HTTP proxy authentication rejection
- Config-to-base64 and base64-to-config CLI conversion for both binaries
- Update-check CLI path with a deterministic local release metadata fixture
- JSON logging configuration during process startup
- Admin health/status/metrics endpoint checks
- Admin `/apply` hot reload, verified by changing remote egress policy at
  runtime and checking that a new proxy request is rejected
- Egress allow-port policy in the remote test config
- Per-user metrics labels in remote metrics
- Profile URL export/import smoke check

## TCP Stress Probe

`scripts/stress_smoke.sh` and `scripts/stress_smoke.ps1` exercise the TCP-first
performance path after the basic e2e probe passes:

- One download-like stream through SOCKS5.
- Many small requests with configurable concurrency.
- Mixed SOCKS5 and HTTP proxy requests so interactive and bulk lanes are both
  used.
- Remote endpoint restart followed by new request recovery.
- Optional soak loop. On Unix-like hosts, set `SOAK_SECS=1800` for a 30-minute
  run.
- Native mux alpha coverage. Set `MUX_MODE=native` on the e2e or stress scripts
  to run the same proxy checks through the in-tree mux instead of yamux.
  Unit tests also cover native max-stream enforcement, idle session GOAWAY, and
  byte-window write blocking until the remote reader releases window.

The fixture returns JSON containing method, path, probe header, and request
body. The script checks that the returned JSON contains the expected token,
path, and body. This proves both directions of the proxy path are working.

## Security/Protocol Unit Tests

Current unit tests cover:

- Variable-length authenticated handshake completion.
- Stealth handshake completion and fixed-size frame roundtrip behavior.
- Client puzzle solving and verification.
- Replay cache duplicate rejection.
- Replay cache expiry.
- UDP underlay packet codec.
- UDP underlay cumulative ACK and retransmission scheduling.
- UDP underlay congestion growth and loss backoff.
- Per-user quota and bandwidth limiter behavior.
- Update metadata version comparison.
- SOCKS5 UDP ASSOCIATE packet wrapping for chained UDP egress.
- Admin authorization and request length parsing.
- Configuration validation for TUN, DNS route, and duplicate users.
- Local SOCKS5 UDP parser boundary cases.
- Transport idle timeout behavior.

## Fuzz Targets

The `fuzz/` crate is intentionally excluded from the main workspace so normal
CI and release builds stay deterministic. Install `cargo-fuzz`, then run:

```bash
cargo fuzz run socks5_udp_packet
cargo fuzz run config_toml
```

The current targets cover the SOCKS5 UDP packet parser and TOML configuration
parser/validator.

## Regression Areas

When changing frame encoding, handshake layout, mux integration, HTTP/SOCKS5
ingress, or configuration loading, always run the full automated check list.

## Not Covered Yet

- Multi-hour soak tests.
- Cross-platform packet-loss simulation.
- Production UDP physical-underlay socket integration.
- Multi-packet UDP underlay packet-loss simulation.
- Live physical tunnel migration.
- Browser/WASM transport behavior.
