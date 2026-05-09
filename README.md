# Espejismo

Espejismo is a native cross-platform Rust privacy tunnel for encrypted
transport on public and untrusted networks. It provides SOCKS5 and HTTP local ingress,
an authenticated variable-length 0-RTT-style handshake, X25519 forward secrecy,
HKDF key derivation, XChaCha20-Poly1305 encrypted frames, optional encrypted
padding frames, client puzzles, replay protection, adaptive padding backpressure,
yamux multiplexing over one encrypted physical tunnel, and sender-side jitter.

The project is designed as a compact, auditable foundation for learning,
research, and lawful private-network deployment. Its public wording focuses on
protecting data confidentiality and metadata minimization in shared network
environments.

## Architecture

`espejismo-local` runs on the user's own device or gateway. It listens on local
SOCKS5 and/or HTTP proxy ports, accepts clear local application requests,
authenticates to the remote endpoint, and sends encrypted framed traffic over a
single TCP stream. Multiple local proxy connections are opened as yamux logical
streams over that one encrypted transport.

`espejismo-core` owns the shared protocol: PSK timestamp authentication,
variable-length first packets, X25519 key agreement, HKDF session derivation,
encrypted frames, padding frames, proof-of-work puzzles, replay protection,
adaptive padding backpressure, a frame-to-`AsyncRead` transport adapter, and
send-side jitter.

`espejismo-remote` runs on a server. It accepts authenticated tunnel sessions,
reassembles and decrypts frames, accepts yamux logical streams, opens the
requested TCP destinations, and sends responses back through the same encrypted
framing layer.

Invalid or incomplete handshakes receive no application-layer response. The
remote can optionally apply a short bounded silent delay before closing the TCP
connection, which avoids leaking protocol details without creating unbounded
resource retention.

## Handshake

The client first packet is intentionally variable length:

```text
[ HMAC-SHA256 32 ][ UTC timestamp 8 ][ nonce 24 ][ X25519 public key 32 ][ padding length 2 ][ padding 0..N ]
```

The current packet body also includes an 8-byte puzzle nonce before the padding
length:

```text
[ HMAC-SHA256 32 ][ UTC timestamp 8 ][ nonce 24 ][ X25519 public key 32 ][ puzzle nonce 8 ][ padding length 2 ][ padding 0..N ]
```

The client solves a bounded SHA-256 leading-zero puzzle over the body before it
computes the HMAC. The remote verifies the puzzle first, then checks the
timestamp skew, validates the HMAC in constant time, and keeps a bounded
in-memory replay cache of recently seen ephemeral public keys.

More detail lives in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Platform Support

Espejismo is implemented in safe Rust and uses Tokio's portable TCP runtime. It
does not require kernel modules, TUN/TAP devices, packet capture permissions, or
C/C++ native extensions.

| Platform | Status | Notes |
| --- | --- | --- |
| Linux x86_64/aarch64 | Supported | Suitable for servers, desktops, and ARM single-board systems. |
| macOS Intel/Apple Silicon | Supported | Build with the standard Rust toolchain. |
| Windows x86_64/aarch64 | Supported | Run from PowerShell or Windows Terminal. |

## Build

```bash
cargo build --release
```

Cross-platform CI checks Linux, macOS, and Windows.

## Run on Linux/macOS

Terminal 1:

```bash
ESPEJISMO_PSK='change-me-long-random-secret' \
cargo run --bin espejismo-remote -- --listen 0.0.0.0:8443
```

Terminal 2:

```bash
ESPEJISMO_PSK='change-me-long-random-secret' \
cargo run --bin espejismo-local -- \
  --socks5-listen 127.0.0.1:1080 \
  --http-listen 127.0.0.1:8080 \
  --server 127.0.0.1:8443
```

Then point a SOCKS5-capable client at `127.0.0.1:1080` or an HTTP proxy client
at `127.0.0.1:8080`.

## Run on Windows PowerShell

Terminal 1:

```powershell
$env:ESPEJISMO_PSK = "change-me-long-random-secret"
cargo run --bin espejismo-remote -- --listen 127.0.0.1:8443
```

Terminal 2:

```powershell
$env:ESPEJISMO_PSK = "change-me-long-random-secret"
cargo run --bin espejismo-local -- --socks5-listen 127.0.0.1:1080 --http-listen 127.0.0.1:8080 --server 127.0.0.1:8443
```

## Configuration

Generate a starter TOML config:

```bash
cargo run --bin espejismo-local -- --print-example-config > espejismo.toml
```

The same config can be used by both binaries; each one reads the relevant
section.

```toml
[shared]
psk = "change-me-long-random-secret"
clock_skew_secs = 30
puzzle_bits = 12
max_padding = 64
jitter_ms = 0
padding_chance_percent = 35
backpressure_threshold_ms = 40
backpressure_cooldown_ms = 1000
tunnel_buffer = 1048576

[local]
server = "127.0.0.1:8443"
socks5_listen = "127.0.0.1:1080"
http_listen = "127.0.0.1:8080"
handshake_padding = 256

[remote]
listen = "0.0.0.0:8443"
handshake_timeout_ms = 3000
reject_delay_ms = 0
max_handshake_padding = 1024
replay_window_secs = 60
cold_start_delay_ms = 35
tarpit_max = 1024
tarpit_hold_secs = 300
```

Run from a file:

```bash
cargo run --bin espejismo-remote -- --config espejismo.toml
cargo run --bin espejismo-local -- --config espejismo.toml
```

Run from base64-encoded TOML, useful for deployment panels or one-line imports:

```bash
CONFIG_B64="$(base64 -w0 espejismo.toml)"
cargo run --bin espejismo-remote -- --config-base64 "$CONFIG_B64"
cargo run --bin espejismo-local -- --config-base64 "$CONFIG_B64"
```

You can print an example directly as base64:

```bash
cargo run --bin espejismo-local -- --print-example-config-base64
```

## Notes

- `espejismo-local --socks5-listen` enables the local SOCKS5 proxy.
- `espejismo-local --http-listen` enables the local HTTP proxy.
- `--max-padding` controls the maximum payload size of encrypted padding frames.
- `--padding-chance-percent` controls how often padding is attempted.
- `--backpressure-threshold-ms` detects slow writes and disables padding.
- `--backpressure-cooldown-ms` controls how long padding stays disabled after a
  slow write.
- `--jitter-ms` applies a small randomized delay before outgoing frames.
- `--puzzle-bits` configures the client puzzle difficulty. Values are capped at
  24 bits.
- `espejismo-local --handshake-padding` controls the maximum random padding in
  the first packet.
- `espejismo-remote --max-handshake-padding` limits accepted first-packet
  padding.
- `espejismo-remote --replay-window-secs` controls the in-memory replay cache
  window.
- `espejismo-remote --handshake-timeout-ms` bounds incomplete handshakes.
- `espejismo-remote --reject-delay-ms` adds a bounded silent close delay for
  invalid handshakes. Values above 10000 ms are capped.
- `espejismo-remote --tarpit-max` controls the bounded silent tarpit size used
  when `reject_delay_ms = 0`.
- `espejismo-remote --tarpit-hold-secs` controls how long invalid sockets are
  retained in the bounded silent tarpit.
- `--tunnel-buffer` controls the in-process encrypted transport buffer used
  below yamux.
- `espejismo-remote --cold-start-delay-ms` applies a small startup delay after
  a valid handshake and before yamux begins.
- The PSK accepts `hex:...`, `base64:...`, or a raw UTF-8 string.
- Invalid handshakes are closed quietly and without application data.
- The tarpit is intentionally silent: it holds sockets briefly and never sends
  drip bytes to unknown peers.

## Smoke Test

```bash
./scripts/e2e_smoke.sh
```

The script starts a local HTTP server, `espejismo-remote`, and `espejismo-local`,
then performs two SOCKS5 requests through the same encrypted yamux tunnel.

## Project Status

See [docs/development/STATUS.md](docs/development/STATUS.md) for the implemented
feature matrix and the remaining roadmap, including UDP, transparent migration,
WASM/browser packaging, metrics, and runtime reload.

See [docs/testing/TEST_PLAN.md](docs/testing/TEST_PLAN.md) for the executable
test strategy and [docs/research/DESIGN_PRINCIPLES.md](docs/research/DESIGN_PRINCIPLES.md)
for the protocol design principles.
