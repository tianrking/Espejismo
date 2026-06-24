# Configuration

Espejismo uses one TOML shape for both binaries. You may keep one file and pass
it to both sides:

```bash
espejismo-remote --config espejismo.toml
espejismo-local --config espejismo.toml
```

The remote binary reads `shared`, `remote`, `logging`, and `admin`. The local
binary reads `shared`, `local`, `logging`, and `admin`. Unknown sections are not
needed by that role but are harmless.

## Minimal Server And Client

Use this as the shortest real deployment config:

```toml
[shared]
psk = "change-me-to-a-long-random-secret"

[shared.mux]
mode = "yamux"

[local]
server = "203.0.113.10:6690"
socks5_listen = "127.0.0.1:6680"
http_listen = "127.0.0.1:6681"

[remote]
listen = "0.0.0.0:6690"

[remote.egress]
deny_private_ips = true
allow_ports = [80, 443]

[logging]
level = "info"
format = "compact"

[admin]
listen = "127.0.0.1:9090"
token = "change-me-admin-token"
```

On the server, set `remote.listen` to the public bind address and open that TCP
port in your firewall. On the client, set `local.server` to the public address
clients can dial.

If `remote.users` is empty, the remote authenticates with `shared.psk`. If
`remote.users` is configured, each user has its own PSK and the client must use
the matching PSK in `shared.psk`.

## Full Example

The maintained one-file example is:

```text
configs/examples/espejismo.toml
```

Generate the same shape from a binary:

```bash
espejismo-local --print-example-config > espejismo.toml
```

Validate before running:

```bash
espejismo-remote --config espejismo.toml --check-config
espejismo-local --config espejismo.toml --check-config
```

## Accepted Config Parameters

### shared

`shared.psk`: Shared secret for single-user mode and local client profiles.
Minimum length is 16 bytes.

`shared.clock_skew_secs`: Accepted handshake timestamp skew.

`shared.puzzle_bits`: SHA-256 client puzzle difficulty.

### shared.handshake_window

Dynamic handshake windows bind the first packet authentication key to time:

```text
handshake_auth_key = HKDF(PSK, "espejismo v1 handshake-window-auth-key" || floor(unix_time / step_secs))
```

The client sends with the current window. The server tries the current window
plus the configured previous/future tolerance. A recorded first packet replayed
after the accepted window set expires no longer decrypts as a valid hello and
falls into the normal silent reject/tarpit path.

Recommended production baseline:

```toml
[shared.handshake_window]
enabled = true
step_secs = 30
previous_windows = 1
future_windows = 0
```

`enabled`: Enable dynamic first-packet handshake authentication keys. Keep this
the same on client and server.

`step_secs`: Window size in seconds. `30` is the default. Smaller values reduce
replay lifetime but require tighter client/server clocks.

`previous_windows`: Number of older windows accepted by the server. `1` allows
roughly one extra step for latency and small clock skew.

`future_windows`: Number of future windows accepted by the server. Keep `0`
unless client clocks are known to run ahead. Increasing this widens the replay
tolerance.

`shared.max_padding`: Maximum normal-mode random padding bytes.

`shared.jitter_ms`: Optional send jitter ceiling.

`shared.padding_chance_percent`: Chance to send padding before a data frame.

`shared.backpressure_threshold_ms`: Write latency threshold that disables
padding temporarily.

`shared.backpressure_cooldown_ms`: Padding cooldown after backpressure.

`shared.tunnel_buffer`: In-process frame transport buffer size.

`shared.idle_timeout_secs`: Idle copy timeout for streams.

`shared.max_streams`: Concurrent logical stream limit.

`shared.max_physical_connections`: Remote physical TCP connection cap.

`shared.key_update_frames`: Frame interval for AEAD traffic-key rotation.

### shared.tcp

`nodelay`: Enable TCP_NODELAY.

`keepalive_secs`: TCP keepalive interval.

`heartbeat_secs`: Encrypted heartbeat interval.

`user_timeout_ms`: Linux TCP_USER_TIMEOUT, 0 disables it.

`send_buffer_bytes` / `recv_buffer_bytes`: Socket buffer sizes, 0 leaves OS
defaults.

`congestion_control`: Optional OS TCP congestion-control algorithm name.

### shared.mux

`mode`: `yamux` for production, `native` for the in-tree beta mux.

`native_initial_window_bytes`: Native mux per-stream flow-control window.

`native_stream_buffer_frames`: Native mux bounded receive queue.

`native_send_queue_frames`: Native mux bounded send queue.

`native_idle_timeout_secs`: Native mux idle GOAWAY timeout.

`native_drain_timeout_secs`: Native mux GOAWAY drain window.

### shared.pacing

`enabled`: Enable application-level pacing.

`max_bytes_per_sec`: Rate cap. `0` means uncapped.

`burst_bytes`: Uncharged burst budget.

`min_write_bytes`: Minimum pacing write charge.

### shared.obfuscation

`profile`: `low_latency`, `balanced`, `high_entropy`, `bulk`, or `stealth`.

`chunk_policy`: `low_latency`, `balanced`, `bulk`, `stealth`, or `custom`.

`randomize_chunks`: Randomize normal-mode data chunk sizes.

`min_chunk` / `max_chunk`: Chunk bounds for `custom`, and the operator-selected
ceiling for `bulk`. Normal non-stealth frames can carry up to 262127 bytes of
payload. Bulk mode defaults to at least 64 KiB chunks; for high-BDP links, set
`chunk_policy = "bulk"`, `randomize_chunks = false`, and raise `max_chunk`
to `131072` or `262127` on both peers to reduce per-frame overhead. Stealth
frames remain controlled by `[shared.stealth]` and should stay small for cover
traffic.

### shared.stealth

`frame_size`: Fixed stealth handshake frame size and fallback data frame size.

`frame_size_candidates`: Optional fixed-size candidate list. When set, each
authenticated session picks one data frame size deterministically.

`tick_ms`: Base stealth pacing tick.

### local

`server`: Remote endpoint in `host:port` form.

`socks5_listen`: Local SOCKS5 listener, usually `127.0.0.1:6680`.

`http_listen`: Local HTTP proxy listener, usually `127.0.0.1:6681`.

`handshake_padding`: Client hello padding cap.

`http_bulk_threshold_bytes`: HTTP proxy requests with `Content-Length` greater
than or equal to this value are opened on bulk lanes. The default is `1048576`
(1 MiB). Set `0` to keep all HTTP proxy streams on interactive lanes.

### local.auth

Optional local proxy authentication:

```toml
[local.auth]
username = "local-user"
password = "local-pass"
```

When local listeners bind only to localhost, leaving this disabled is usually
fine for single-user desktop use.

### local.tunnel_pool

`min_connections`: Minimum physical lanes.

`max_connections`: Maximum physical lanes.

`interactive_lanes`: Preferred lanes for interactive streams.

`bulk_lanes`: Preferred lanes for bulk streams.

`max_reconnect_attempts`: Per-open reconnect attempts.

`max_connection_age_secs`: Rotate old physical tunnels for new streams.

Suggested desktop/browser baseline:

```toml
[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 2
bulk_lanes = 2
max_reconnect_attempts = 3
max_connection_age_secs = 3600
```

Use more `interactive_lanes` for browser SOCKS5/HTTP proxy workloads. Use more
`bulk_lanes` for TUN-heavy or transfer-heavy workloads. Each lane is an
independent TCP physical tunnel, so packet loss on one lane does not block
logical streams placed on other lanes.

High-throughput TCP baseline:

```toml
[shared.obfuscation]
profile = "bulk"
chunk_policy = "bulk"
randomize_chunks = false
min_chunk = 65536
max_chunk = 262127

[shared.pacing]
enabled = true
burst_bytes = 524288
min_write_bytes = 65536

[shared.mux]
native_initial_window_bytes = 8388608

[local.tunnel_pool]
min_connections = 1
max_connections = 8
interactive_lanes = 1
bulk_lanes = 4
```

The same shape can be applied as an official overlay:

```bash
espejismo-remote --profile auto-throughput --config server.toml
espejismo-local --profile auto-throughput --config client.toml
```

`auto-throughput` raises normal-frame chunks to the exact 262127-byte payload
cap, enables 16 MiB tunnel/mux buffering, requests 4 MiB TCP socket buffers,
sets `http_bulk_threshold_bytes = 262144`, and uses one interactive lane plus
six bulk lanes. Use it for measured long-haul throughput tests, not as a stealth
traffic-shaping profile.

### local.tun

`enabled`: Enable native TUN ingress.

`name`: TUN interface name.

`address`: Local TUN IPv4 address.

`prefix`: TUN IPv4 prefix length.

`destination`: Peer/gateway IPv4 address for the TUN interface.

`mtu`: Interface MTU.

`udp_enabled`: Enable UDP datagram relay from TUN. Set to `false` for the most
stable TCP-only global TUN mode.

`udp_timeout_secs`: Per-datagram response timeout for UDP relay.

`udp_block_ports`: UDP destination ports dropped locally before relay. Defaults
to `[443]` so QUIC falls back to TCP HTTPS; use `[]` to allow UDP/443.

### local.tun.route

`enabled`: Install route takeover rules.

`protect_server_route`: Keep direct route to `local.server`.

`dns_enabled`: Apply DNS takeover.

`dns_servers`: DNS servers to apply when DNS takeover is enabled.

### remote

`listen`: Public remote listener.

`handshake_timeout_ms`: Handshake timeout for unauthenticated peers.

`reject_delay_ms`: Silent rejection delay. `0` uses the bounded tarpit.

`max_handshake_padding`: Remote accepted client hello padding cap.

`replay_window_secs`: Replay cache window for client ephemeral keys.

`cold_start_delay_ms`: Delay after successful auth before tunnel startup.

`tarpit_max`: Maximum sockets in silent tarpit.

`tarpit_hold_secs`: Tarpit hold duration.

### remote.fallback_http

`mode`: `silent` or `http_fallback`.

`enabled`: Legacy fallback switch.

`upstream`: Optional fallback upstream endpoint.

`probe_timeout_ms`: HTTP probe peek timeout.

`server`: Built-in fallback `Server` header value.

`body`: Built-in fallback response body.

### remote.users

Optional multi-user credentials:

```toml
[[remote.users]]
name = "alice"
psk = "alice-long-random-secret"

[remote.users.quota]
bytes = 536870912
window_secs = 86400

[remote.users.bandwidth]
bytes_per_sec = 1048576
```

`quota.bytes` and `bandwidth.bytes_per_sec` are optional.

### remote.egress

`deny_private_ips`: Block private, loopback, link-local, and special egress IPs.

`allow_hosts`: Optional host allowlist. Supports `*.example.com`.

`block_hosts`: Optional host blocklist. Supports `*.example.com`.

`allow_ports`: Optional outbound port allowlist.

`block_ports`: Optional outbound port blocklist.

`proxy`: Optional upstream proxy for server-side egress chaining. Supported
forms:

```toml
[remote.egress]
proxy = "socks5://127.0.0.1:1080"
# proxy = "socks://127.0.0.1:1080"
# proxy = "socks4://192.0.2.10:1080"
# proxy = "socks4a://proxy.example.com:1080"
# proxy = "socks5://user:pass@127.0.0.1:1080"
# proxy = "http://127.0.0.1:8080"
# proxy = "http://user:pass@127.0.0.1:8080"
# proxy = "https://proxy.example.com:8443"
# proxy = "https://user:pass@proxy.example.com:8443"
```

`socks://` is an alias for SOCKS5. SOCKS5 supports TCP CONNECT and UDP
ASSOCIATE. SOCKS4 and SOCKS4a support TCP only. HTTP and HTTPS support TCP
CONNECT only; HTTPS means TLS to the upstream proxy before the CONNECT request.

`socks5_proxy`: Legacy alias for a no-auth SOCKS5 chain. Prefer `proxy` for new
deployments.

### logging

`level`: Tracing filter such as `info`, `debug`, or module filters.

`format`: `compact`, `pretty`, or `json`.

`file`: Optional log file path.

`ansi`: Enable ANSI color when writing to stderr.

### admin

`listen`: Optional admin HTTP listener.

`token`: Admin bearer token. Required when `admin.listen` is not loopback.

Supported endpoints include `/healthz`, `/status`, `/connections`, `/metrics`,
`/reload`, and `/apply`.

## Installer Parameters

The release download scripts accept these environment variables:

```bash
ESPEJISMO_REPO=tianrking/Espejismo
ESPEJISMO_VERSION=latest
ESPEJISMO_PACKAGE=full
ESPEJISMO_INSTALL_DIR=$HOME/.espejismo
ESPEJISMO_ARCHIVE_URL=https://example.com/espejismo-linux-amd64.tar.gz
ESPEJISMO_OS=linux
ESPEJISMO_ARCH=amd64
```

`ESPEJISMO_PACKAGE` can be `full` or `server`. `ESPEJISMO_OS` and
`ESPEJISMO_ARCH` are normally auto-detected and are mainly for testing or custom
packaging mirrors.

Supported release artifact suffixes:

```text
linux-amd64
linux-386
linux-arm64
linux-armv7
darwin-arm64
windows-amd64
windows-386
windows-arm64
```
