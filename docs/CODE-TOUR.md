# Code Tour

This guide helps contributors find the implementation behind Espejismo's two
binaries, shared protocol, and supporting tools. Start with the workspace map,
then follow the client or server path that matches the behavior you want to
understand. For wire-level contracts, use [the protocol specification](PROTOCOL.md);
for runtime relationships, use [the architecture guide](ARCHITECTURE.md).

## Workspace map

The root `Cargo.toml` defines a four-member Rust workspace:

- `crates/espejismo-core` is the shared library. It owns configuration,
  cryptographic handshake and session keys, protocol framing, ingress parsers,
  multiplexing interfaces, underlays, egress policy, metrics, and shared
  runtime helpers. Its `src/lib.rs` is the public module and re-export map.
- `crates/espejismo-client` builds `espejismo-local`. Its `main.rs` parses CLI
  options and starts listeners; neighboring modules implement proxy handlers,
  tunnel pool management, mux integration, adaptive lane selection, and
  optional TUN routing.
- `crates/espejismo-server` builds `espejismo-remote`. Its `main.rs` owns
  configuration and listener startup; its modules handle authenticated peers,
  mux streams, resource limits, destination relay, proxy chaining, and fallback
  behavior.
- `crates/tokio-yamux` contains the workspace's yamux implementation: session,
  stream, control, configuration, frame, and error handling.

The `fuzz/` directory is a separate cargo-fuzz package excluded from the main
workspace. `scripts/` contains developer and packaging helpers; `configs/` has
the maintained example configuration; `deployments/` contains Docker and
systemd assets.

## Follow a connection

### Client

1. Begin at `crates/espejismo-client/src/main.rs`: CLI and config handling,
   startup validation, and listener orchestration.
2. `handler.rs` accepts SOCKS5 and HTTP proxy clients and turns requests into
   tunnel work. `tun.rs` and `route/` cover optional virtual-interface capture
   and platform route/DNS setup.
3. `tunnel.rs` manages authenticated physical lanes and stream assignment;
   `mux.rs` connects logical streams to the configured mux, while `adaptive.rs`
   contains lane selection behavior.
4. Shared handshake, frame transport, and underlay primitives are in
   `espejismo-core` (`crypto/`, `protocol/`, `transport/`, and `underlay.rs`).

### Server

1. Begin at `crates/espejismo-server/src/main.rs` for CLI/configuration,
   listener startup, and shared runtime services.
2. `handler.rs` authenticates a physical peer and dispatches its mux session;
   `mux.rs` receives logical streams.
3. Stream request parsing and egress policy live in core
   (`protocol/request.rs` and `egress.rs`). Server `relay.rs` performs outbound
   TCP/UDP work; `http_chain.rs` and `socks5_chain.rs` implement upstream proxy
   paths.
4. `limits.rs` and `tarpit.rs` contain server-side resource controls and
   unsuccessful-handshake handling. `fallback.rs` and `http_chain.rs` support
   the configured fallback and proxy behavior.

## Core module landmarks

| Path | Responsibility |
| --- | --- |
| `espejismo-core/src/config/` | Config types, defaults, parsing, validation, and profiles |
| `espejismo-core/src/crypto/` | Authenticated X25519 handshake and derived session keys |
| `espejismo-core/src/protocol/` | Frame codec, puzzle, replay cache, requests, and UDP primitives |
| `espejismo-core/src/transport/` and `underlay.rs` | Async frame transport and TCP/WebSocket/HTTP/2 adapters |
| `espejismo-core/src/mux/` | Mux mode abstraction and native mux implementation |
| `espejismo-core/src/ingress/` | SOCKS5 and HTTP proxy parsing/authentication |
| `espejismo-core/src/egress.rs` and `dns.rs` | Destination policy, proxy selection, and name resolution |
| `espejismo-core/src/admin.rs`, `metrics.rs`, `runtime_state.rs` | Admin API and process observability/state |

`espejismo-core/src/lib.rs` re-exports common APIs used by both binaries. When
tracing a behavior, follow the call from a binary into those public APIs and
then into the owning module rather than assuming every implementation lives in
the binary crate.

## Where to look next

- [Architecture](ARCHITECTURE.md) explains runtime ownership and data flow.
- [Protocol specification](PROTOCOL.md) defines the wire contract.
- [Development index](development/INDEX.md) links contribution, validation,
  security, and performance guidance.
- [Project positioning](POSITIONING.md) sets product boundaries: Espejismo
  does not impersonate other protocols and keeps a small operational model.
