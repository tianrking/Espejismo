# Configuration

For the authentication flow, PSK handling, and credential-rotation procedure,
see [Authentication and Key Management](AUTHENTICATION.md).

For workload-oriented profile selection, parameter trade-offs, and a repeatable
throughput tuning process, see [Performance Tuning](PERFORMANCE.md).

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

## Complete Scenario Examples

These examples are complete single-file configs: put the same file on the
remote and local machines, then run the binary for that role. Each binary
ignores the other role's section. Replace every example PSK and admin token
with deployment-specific random values before use.

### One user with a local SOCKS5 proxy

This is the usual single-user VPS setup. The proxy listens only on loopback;
applications on the client machine connect to `127.0.0.1:6680`.

```toml
[shared]
psk = "replace-with-a-long-random-secret"

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
```

Allow TCP port 6690 through the server firewall. The server and client must
use the same `shared.psk`.

### Multiple users with per-user limits

For a small shared server, keep the user list on the remote and give each
client its own `shared.psk`. The remote matches that key to the user's entry;
do not put other users' secrets in a client's config.

```toml
[shared]
psk = "alice-long-random-secret"

[local]
server = "203.0.113.10:6690"
socks5_listen = "127.0.0.1:6680"
http_listen = "127.0.0.1:6681"

[remote]
listen = "0.0.0.0:6690"

[[remote.users]]
name = "alice"
psk = "alice-long-random-secret"

[remote.users.quota]
bytes = 5368709120
window_secs = 2592000

[remote.users.bandwidth]
bytes_per_sec = 10485760

[[remote.users]]
name = "bob"
psk = "bob-long-random-secret"

[remote.users.quota]
bytes = 10737418240
window_secs = 2592000

[remote.users.bandwidth]
bytes_per_sec = 20971520

[remote.egress]
deny_private_ips = true
allow_ports = [80, 443]

[logging]
level = "info"
format = "compact"
```

Install the same remote user list on the server. Alice's client uses Alice's
PSK and Bob's client uses Bob's PSK. Quotas are in bytes per `window_secs`;
bandwidth is bytes per second. Omit either limit table value when that limit
is not needed.

### TUN client with route and DNS takeover

Use this when applications should use the tunnel without per-application
proxy settings. The route settings apply on the local machine; run with the
permissions needed to create a TUN interface and change routes. Confirm that
the server address remains reachable directly before enabling route takeover.

```toml
[shared]
psk = "replace-with-a-long-random-secret"

[local]
server = "203.0.113.10:6690"
socks5_listen = "127.0.0.1:6680"
http_listen = "127.0.0.1:6681"

[local.tun]
enabled = true
name = "esptun0"
address = "10.255.0.2"
prefix = 24
destination = "10.255.0.1"
mtu = 1400
udp_enabled = false

[local.tun.route]
enabled = true
protect_server_route = true
dns_enabled = true
dns_servers = ["1.1.1.1", "8.8.8.8"]

[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 1
bulk_lanes = 2

[remote]
listen = "0.0.0.0:6690"

[remote.egress]
deny_private_ips = true
allow_ports = [80, 443]

[logging]
level = "info"
format = "compact"
```

The server uses the same `shared.psk`; it does not need the client's TUN
settings. This example disables UDP relay for a TCP-only baseline. Keep route
takeover disabled until the interface and direct server route have been
checked on the target operating system.

## Full Example

The maintained one-file example is:

```text
configs/examples/espejismo.toml
```

The `espejismo-core` crate doctest parses and serializes this exact file, so
`cargo test --doc -p espejismo-core` checks the documented configuration against
the parser without network access.

Generate the same shape from a binary:

```bash
espejismo-local --print-example-config > espejismo.toml
```

Validate before running:

```bash
espejismo-remote --config espejismo.toml --check-config
espejismo-local --config espejismo.toml --check-config
```

### Startup validation and errors

Configuration is checked in stages. File and base64 input must first decode as
UTF-8 TOML and match the known configuration fields and types. Unknown keys
are errors (the diagnostic may suggest a close spelling); this helps catch
misspellings and options removed by an upgrade. The parser then checks
cross-field constraints and supported ranges. For example, TUN prefix must be
`0..=32`, MTU at least `576`, stream and physical connection limits
`1..=65535`, and the tunnel pool must have at least one lane with its lane sum
no greater than `max_connections`. Conditional checks apply when a feature is
enabled: DNS takeover needs at least one DNS server, pacing needs positive
burst and minimum-write sizes, and enabled port hopping needs unique nonzero
ports, a positive window, and a nonempty seed. Errors name the field or fields
to correct and often include the accepted range or an example value.

Parsing does not prove that the process can start on this machine. The
role-specific `--check-config` command also checks required role settings,
resolves `local.server`, checks bindability of configured proxy/admin/tunnel
listeners, and detects listener address reuse. Server checks require either
`remote.users` or a shared PSK; client checks require `local.server` and a PSK.
These checks report `ERROR` for blockers and return a failure status. `WARNING`
messages identify advisory conditions such as a short PSK or broad egress
policy; they do not by themselves fail the check. `--doctor` includes these
checks and adds reachability and feature diagnostics, so network-dependent
warnings can reflect the current environment. A successful check is a snapshot:
DNS answers, port availability, permissions, and remote reachability can
change before the next startup.

For example, a message such as `unknown config field` points to a TOML key to
rename or remove; `must be in ...` or `must be greater than 0` identifies a
value constraint; `cannot bind` means the address is occupied or unavailable
to this process; and `cannot resolve` means the configured hostname did not
resolve during the check. Fix the indicated input or environment issue, then
rerun the check for the same binary role.

## Accepted Config Parameters

### Defaults when fields are omitted

These are the parser defaults from `EspejismoConfig::default()`. An absent
optional credential or endpoint remains unset; `shared.psk` and `local.server`
must be supplied for their respective roles. Explicit values in a TOML file or
an applied profile take precedence.

| Section | Field | Default |
| --- | --- | --- |
| `shared` | `clock_skew_secs`, `puzzle_bits` | `30`, `12` |
| `shared` | `handshake_window.enabled`, `step_secs`, `previous_windows`, `future_windows` | `true`, `30`, `1`, `0` |
| `shared` | `max_padding`, `jitter_ms`, `padding_chance_percent` | `64`, `0`, `35` |
| `shared` | `backpressure_threshold_ms`, `backpressure_cooldown_ms` | `40`, `1000` |
| `shared` | `tunnel_buffer`, `idle_timeout_secs`, `max_streams`, `max_physical_connections`, `key_update_frames` | `1048576`, `300`, `256`, `1024`, `16384` |
| `shared.tcp` | `nodelay`, `keepalive_secs`, `heartbeat_secs` | `true`, `30`, `30` |
| `shared.tcp` | `user_timeout_ms`, `send_buffer_bytes`, `recv_buffer_bytes`, `congestion_control` | `0`, `0`, `0`, unset |
| `shared.mux` | `mode`, `native_initial_window_bytes`, `native_stream_buffer_frames`, `native_send_queue_frames`, `native_idle_timeout_secs`, `native_drain_timeout_secs` | `yamux`, `8388608`, `128`, `64`, `300`, `30` |
| `shared.pacing` | `enabled`, `max_bytes_per_sec`, `burst_bytes`, `min_write_bytes` | `true`, `0` (uncapped), `65536`, `1024` |
| `shared.obfuscation` | `profile`, `chunk_policy`, `randomize_chunks`, `min_chunk`, `max_chunk` | `balanced`, `balanced`, `true`, `4096`, `16384` |
| `shared.stealth` | `frame_size`, `frame_size_candidates`, `tick_ms` | `4096`, `[]`, `50` |
| `shared.stealth_shaper` | `enabled`, `mode`, `idle_noise`, `padding_budget_bps` | `false`, `web`, `poisson`, `0` |
| `shared.stealth_shaper` | `min_delay_ms`, `max_delay_ms`, `idle_max_delay_ms` | `20`, `80`, `1000` |
| `shared.underlay` | `mode` | `tcp` |
| `shared.underlay.websocket` | `path`, `max_frame_bytes`, `host` | `"/espejismo"`, `1048576`, unset |
| `shared.underlay.http2` | `path`, `authority`, `initial_stream_window_bytes`, `initial_connection_window_bytes`, `max_frame_bytes` | `"/espejismo"`, unset, `8388608`, `16777216`, `65536` |
| `shared.port_hopping` | `enabled`, `ports`, `window_secs`, `seed` | `false`, `[]`, `300`, `"espejismo-port-hop"` |
| `local` | `server`, `socks5_listen`, `http_listen` | unset, `127.0.0.1:6680`, `127.0.0.1:6681` |
| `local` | `handshake_padding`, `http_bulk_threshold_bytes`, `auth` | `256`, `1048576`, unset |
| `local.tunnel_pool` | `min_connections`, `max_connections`, `interactive_lanes`, `bulk_lanes` | `1`, `4`, `1`, `2` |
| `local.tunnel_pool` | `max_reconnect_attempts`, `max_connection_age_secs` | `3`, `3600` |
| `local.tun` | `enabled`, `name`, `address`, `prefix`, `destination`, `mtu` | `false`, `esptun0`, `10.255.0.2`, `24`, `10.255.0.1`, `1500` |
| `local.tun` | `udp_enabled`, `udp_timeout_secs`, `udp_block_ports` | `true`, `3`, `[443]` |
| `local.tun.route` | `enabled`, `protect_server_route`, `dns_enabled`, `dns_servers` | `false`, `true`, `false`, `[1.1.1.1, 8.8.8.8]` |
| `remote` | `listen`, `handshake_timeout_ms`, `reject_delay_ms`, `max_handshake_padding` | `0.0.0.0:6690`, `3000`, `0`, `1024` |
| `remote` | `replay_window_secs`, `cold_start_delay_ms`, `tarpit_max`, `tarpit_hold_secs` | `60`, `35`, `1024`, `300` |
| `remote.fallback_http` | `mode`, `enabled`, `upstream`, `probe_timeout_ms` | `silent`, `false`, unset, `250` |
| `remote.fallback_http` | `server`, `body` | `nginx`, built-in “It works” HTML page |
| `remote.users[].quota` | `bytes`, `window_secs` | unset, `86400` |
| `remote.users[].bandwidth` | `bytes_per_sec` | unset |
| `remote.egress` | `deny_private_ips`, host/port lists, `proxy`, `socks5_proxy` | `false`, empty, unset, unset |
| `logging` | `level`, `format`, `file`, `ansi` | `info`, `compact`, unset, `true` |
| `admin` | `listen`, `token` | unset, unset |

The table describes omitted fields, not the values selected by built-in
profiles. For example, the `auto-throughput` profile overlays several buffer,
chunk, socket, threshold, and lane settings described below.

### Tuning guidance

Keep the defaults as the starting point. Change one group at a time and compare
latency, throughput, memory use, and reconnect behavior on the actual path;
larger buffers and more lanes consume more memory and do not guarantee higher
throughput. Settings that must agree across peers are marked below.

| Goal | Settings to consider | Guidance |
| --- | --- | --- |
| High bandwidth-delay product | `shared.obfuscation.chunk_policy`, `randomize_chunks`, `max_chunk`; `shared.tunnel_buffer`; `shared.underlay.http2` windows; `local.tunnel_pool` | Try the documented bulk profile or `auto-throughput` overlay first. Increase chunk sizes/windows or bulk lanes only when measurement shows a throughput ceiling. Keep peer values aligned where the field is shared. |
| Low latency or constrained memory | `local.tunnel_pool.max_connections`, `shared.tunnel_buffer`, `shared.pacing.burst_bytes`, `shared.obfuscation` | Reduce lanes and buffering if memory is constrained. Prefer the `low_latency` chunk policy for small interactive exchanges; validate that bulk transfers remain acceptable. |
| Rate limiting | `shared.pacing.max_bytes_per_sec`; `remote.users[].bandwidth.bytes_per_sec` | Set an application-wide cap with pacing, or a per-user cap on the server. `0` means uncapped for pacing; leave user bandwidth unset for no per-user cap. |
| Replay tolerance | `shared.handshake_window.*`, `shared.clock_skew_secs`, `remote.replay_window_secs` | Keep handshake-window settings identical on both peers. Increase accepted previous windows only for measured clock or path delay; keep the replay cache window at least as large as accepted handshake tolerance. |
| TCP behavior | `shared.tcp.keepalive_secs`, `heartbeat_secs`, `user_timeout_ms`, `send_buffer_bytes`, `recv_buffer_bytes` | Keep OS buffer defaults (`0`) unless measurements or platform guidance indicate otherwise. Shorter keepalive/heartbeat intervals detect dead paths sooner but add traffic; `user_timeout_ms = 0` leaves the OS policy in effect. |
| TUN routing | `local.tun.route.*`, `local.tun.udp_enabled`, `udp_block_ports`, `mtu` | Leave route takeover and DNS takeover disabled until explicitly needed. Protect the server route when takeover is enabled. The default UDP block for port 443 avoids QUIC; clear or change it only when UDP/443 should pass through. |
| Egress restrictions | `remote.egress.deny_private_ips`, `allow_hosts`, `block_hosts`, `allow_ports`, `block_ports` | Set policy to match the server's intended destinations. `deny_private_ips` defaults to `false`; enable it for public-relay deployments that must not reach private or special addresses. Review allow/block rules together before exposure. |
| Stealth shaping | `shared.obfuscation.profile`, `shared.stealth.*`, `shared.stealth_shaper.*` | Use the `stealth` profile on both peers when its traffic pattern is intended. The shaper is disabled by default; idle padding consumes the configured budget and may add latency. Do not treat these settings as protocol camouflage. |
| Port hopping | `shared.port_hopping.*` | Enable only when both peers share the same nonempty seed, port list, and window. Bind/firewall every candidate port on the remote. |
| Diagnostics and admin | `logging.level`, `format`, `file`; `admin.listen`, `token` | Use `info` for routine operation and temporarily increase verbosity to investigate. Enable admin only when needed, keep it on loopback where possible, and use a token for non-loopback binds. |

Optional credentials and endpoints have no default value: configure `shared.psk`
and `local.server` for their roles, and configure admin tokens, egress proxies,
fallback upstreams, user quotas, or bandwidth limits only when required. Avoid
copying example secrets into a deployment.

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

#### Connection and stream limits

These limits apply at different scopes and are independent:

| Setting | Scope | At capacity |
| --- | --- | --- |
| `local.tunnel_pool.max_connections` | Client process; maximum authenticated physical tunnel lanes in its pool | The client does not create additional lanes. New proxy flows use available lanes and may wait/fail according to the lane reconnect and stream-open path. |
| `shared.max_physical_connections` | Remote process; all accepted physical TCP tunnel connections across its listeners | The accept loop drops newly accepted sockets immediately and logs at debug level. Existing connections keep their permits until their handler ends. |
| `shared.max_streams` | Remote process-wide logical streams, and separately per authenticated physical connection | A stream at the process-wide cap causes the peer handler to end, closing that physical connection and its streams. At the per-connection cap, the handler waits up to its bounded permit timeout; if no stream finishes in time, it ends that physical connection. |

`shared.max_streams` therefore bounds aggregate server stream work and also
sets the per-connection ceiling; it is not a per-user quota. Both configured
server limits default to `max_physical_connections = 1024` and `max_streams =
256`. They must be in `1..=65535`; zero is rejected during config validation.
The client pool defaults to `min_connections = 1` and `max_connections = 4`.

The server's physical connection limit also bounds the number of simultaneous
handshakes, since a connection permit is acquired before peer authentication.
`remote.tarpit_max` is a separate cap on connections held by the fallback
tarpit and does not increase the physical connection limit. OS listen backlog
and file-descriptor limits can impose lower effective admission limits.

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

### shared.stealth_shaper

Optional traffic shaper for `profile = "stealth"`. It is disabled by default
unless the built-in `--profile stealth` overlay is applied.

```toml
[shared.stealth_shaper]
enabled = true
mode = "web"
idle_noise = "poisson"
padding_budget_bps = 16384
min_delay_ms = 20
max_delay_ms = 80
idle_max_delay_ms = 1000
```

`enabled`: Enable stealth idle-padding budget and shaped tick timing.

`mode`: `web`, `stream`, or `custom`. `web` slows the idle cadence as a
connection stays quiet. `stream` keeps a tighter steady cadence. `custom` uses
the configured delay window without idle decay.

`idle_noise`: `off`, `uniform`, or `poisson`. `poisson` is the default for
less regular idle timing.

`padding_budget_bps`: Maximum idle padding bytes per second. `0` disables idle
padding while the shaper is enabled. Real data frames are never charged against
this budget.

`min_delay_ms` / `max_delay_ms`: Active stealth tick delay range.

`idle_max_delay_ms`: Upper bound after `web` idle decay.

### shared.underlay

Physical transport wrapper for the Espejismo crypto/mux stream. The default is
raw TCP.

```toml
[shared.underlay]
mode = "websocket" # tcp, websocket, or http2

[shared.underlay.websocket]
path = "/espejismo"
max_frame_bytes = 1048576
# host = "example.com"

[shared.underlay.http2]
path = "/espejismo"
initial_stream_window_bytes = 8388608
initial_connection_window_bytes = 16777216
max_frame_bytes = 65536
# authority = "example.com"
```

`mode`: `tcp`, `websocket`, or `http2`.

`websocket.path`: HTTP Upgrade request path. Client and server must match.

`websocket.max_frame_bytes`: Maximum accepted WebSocket binary frame payload.

`websocket.host`: Optional client-side Host header override. Leave unset to use
the configured `local.server` authority.

`http2.path`: HTTP/2 request path. Client and server must match.

`http2.authority`: Optional client-side `:authority` override. Leave unset to
use the configured `local.server` authority. The HTTP/2 underlay uses cleartext
prior-knowledge h2 on the configured TCP endpoint; put it behind TLS/ALPN at a
reverse proxy if you need browser-like public HTTPS termination.

`http2.initial_stream_window_bytes`: Per-stream HTTP/2 flow-control window.
Use at least `8388608` for high-latency throughput tests.

`http2.initial_connection_window_bytes`: Connection-level HTTP/2 flow-control
window. It must be greater than or equal to `initial_stream_window_bytes`.

`http2.max_frame_bytes`: Maximum HTTP/2 frame payload. Valid range is `16384`
to `16777215`; `65536` is the current throughput-oriented default.

### shared.port_hopping

Optional deterministic port hopping. It is disabled by default.

```toml
[shared.port_hopping]
enabled = true
ports = [6690, 18443, 28443, 38443]
window_secs = 300
seed = "change-me-shared-port-hop-seed"
```

When enabled, the local client selects one port from `ports` for the current
time window and rewrites the configured `local.server` port before opening a
physical lane. The remote binds `remote.listen` plus every configured port, so
the client can move between windows without requiring privileged firewall
mutation.

`ports`: Candidate remote ports. Client and server must match.

`window_secs`: Time window used for deterministic selection.

`seed`: Shared non-empty selector seed. Treat it as deployment-private metadata.

### local

`server`: Remote endpoint in `host:port` form.

`socks5_listen`: Local SOCKS5 listener, usually `127.0.0.1:6680`.

`http_listen`: Local HTTP proxy listener, usually `127.0.0.1:6681`.

`handshake_padding`: Client hello padding cap.

`http_bulk_threshold_bytes`: HTTP proxy requests with `Content-Length` greater
than or equal to this value are opened on bulk lanes. Plain HTTP `GET` requests
whose path looks like a large download, such as `.bin`, `.zip`, `.tar.gz`,
`.mp4`, or `.iso`, also use bulk lanes because downloads usually have no request
body `Content-Length`. The default is `1048576` (1 MiB). Set `0` to disable the
upload-size threshold; filename-style HTTP download classification still
applies.

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

Lane selection is adaptive inside each preferred lane class. New streams prefer
the requested class (`interactive` for small requests, UDP, SOCKS5, HTTPS
CONNECT, and unknown flows; `bulk` for large HTTP uploads and plain HTTP
download paths that look like archives, installers, images, packages, or media)
and then score candidate physical lanes by current load, pending opens,
stream-open failure ratio, last error state, mux RTT trend, open latency, and
recent EWMA throughput. The admin endpoint exposes the score and recent bps
fields per lane; lower scores are healthier. Per-lane byte counters include
bytes from active in-progress streams, so a long download or upload should be
visible before the stream closes.

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
These are server IP addresses for the host operating system's DNS client; they
do not configure an Espejismo resolver or DNS-over-HTTPS (DoH). DNS takeover is
opt-in and disabled by default. See [DNS behavior](DNS.md) for hostname
resolution paths and platform details.

### remote

`listen`: Public remote listener.

`handshake_timeout_ms`: Handshake timeout for unauthenticated peers.

`reject_delay_ms`: Silent rejection delay. `0` uses the bounded tarpit.

`max_handshake_padding`: Remote accepted client hello padding cap.

`replay_window_secs`: Replay cache window for authenticated first client packet
digests and client ephemeral keys. Keep this at least as large as the accepted
handshake-window tolerance so same-window exact replays are rejected before any
server response is sent.

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

`level`: Tracing filter such as `info`, `debug`, or module filters. Global
`debug` and `trace` are treated as Espejismo-only verbosity; high-volume
transport dependencies remain capped at `info`.

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
