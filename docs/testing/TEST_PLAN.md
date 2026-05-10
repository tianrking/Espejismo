# Espejismo Test Plan

## Goals

The test suite must prove that Espejismo works as a real proxy, not merely as a
set of compiling protocol primitives.

The core end-to-end invariant is:

1. A client application sends a request to `espejismo-local`.
2. `espejismo-local` opens a yamux stream over the encrypted physical tunnel.
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
./scripts/e2e_smoke.sh
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
- JSON logging configuration during process startup
- Admin health/status/metrics endpoint checks
- Egress allow-port policy in the remote test config
- Profile URL export/import smoke check

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

## Regression Areas

When changing frame encoding, handshake layout, yamux integration, HTTP/SOCKS5
ingress, or configuration loading, always run the full automated check list.

## Not Covered Yet

- Multi-hour soak tests.
- Cross-platform packet-loss simulation.
- Multi-packet UDP flows and fragmentation behavior.
- Live physical tunnel migration.
- Browser/WASM transport behavior.
