# Performance Tuning

Espejismo uses TCP with Yamux by default. Tune for the actual path and workload:
round-trip time, available bandwidth, loss, CPU, memory, and the mix of short
interactive requests and long transfers all matter. Keep `balanced` as the
general-purpose starting point. For packet-shape requirements, use the
`stealth` profile and accept its smaller frames, shaping traffic, and lower
bulk throughput. Performance profiles do not provide protocol camouflage.

## Start With A Profile

Profiles are config overlays; they do not change Espejismo's authenticated,
encrypted tunnel or its TCP/Yamux architecture. Apply a profile to both peers
where the affected setting is shared:

```bash
espejismo-remote --profile auto-throughput --config server.toml
espejismo-local --profile auto-throughput --config client.toml
```

| Workload | Starting point | Trade-off |
| --- | --- | --- |
| Everyday proxy use | `balanced` (default) | Moderate frame and lane sizes for mixed workloads. |
| Interactive requests, smaller memory budget | `low-latency` | Smaller chunks and burst budget; bulk transfers may be slower. |
| Bulk transfers needing moderate tuning | `fast` | 8 MiB tunnel buffer and four bulk lanes; less aggressive than `auto-throughput`. |
| Measured long-RTT bulk transfers | `auto-throughput` | Larger buffers, maximum normal-frame chunks, and more bulk lanes; uses more memory and reduces padding/jitter. |
| Packet-shape-sensitive operation | `stealth` | Small fixed-size frames and optional shaping add overhead and latency. |
| Restrictive remote egress/resource policy | `server-safe` (remote) | Denies private IPs, limits destination ports and caps resources; may reject destinations clients need. |

`server-safe` targets remote policy controls rather than throughput. The other
named profiles are overlays, not complete replacement configs; explicit config
and CLI values can affect the effective settings. Check the startup output and
the resolved config before comparing runs. See [Profiles](PROFILES.md) for the
full use case of each named overlay.

## Tune One Bottleneck At A Time

1. Measure direct-path capacity and tunnel throughput in both directions. Use
   multiple rounds and compare medians; check each case's success status and
   spread before drawing conclusions.
2. If long-RTT bulk throughput is below path capacity, try `auto-throughput`
   first. It sets normal-frame chunk bounds to 64 KiB through the 262127-byte
   payload cap, disables chunk randomization, raises the tunnel buffer and
   mux window to at least 16 MiB, requests at least 4 MiB TCP socket buffers,
   and configures one interactive plus six bulk lanes (pool max 8).
3. If a single stream remains limited, inspect the path, CPU, retransmissions,
   and flow-control before adding lanes. More lanes primarily help concurrent
   flows; they do not guarantee a faster single flow.
4. Change one setting group at a time and rerun the same workload. Keep a
   change only when repeated results improve without unacceptable latency,
   memory growth, or reconnect behavior.

The named `auto-throughput` profile uses static buffer floors. On clients with
RTT-based adaptive throughput sizing enabled, buffer and window floors are
estimated from RTT for fields left at defaults. The estimate assumes a 1 Gbit/s
target, is bounded to limit memory growth, and does not infer actual path
bandwidth. Explicitly configured values remain operator-controlled. Prefer
measurement over treating either profile values or BDP estimates as universal
optima.

## Knobs And Their Costs

| Setting | What it affects | Tuning notes |
| --- | --- | --- |
| `shared.obfuscation.chunk_policy`, `min_chunk`, `max_chunk`, `randomize_chunks` | Normal encrypted data frame sizing | Larger chunks reduce per-frame overhead on bulk paths. `max_chunk` is bounded by the 262127-byte normal payload capacity. Keep shared obfuscation settings aligned on both peers. Stealth frames use `[shared.stealth]` instead. |
| `shared.tunnel_buffer` | Buffered transport data | More buffering can help keep high-BDP paths full, at a memory cost. Avoid increasing it without a measured throughput ceiling. |
| `shared.mux.mode` | Multiplexer implementation | `yamux` is the production default and recommended choice; `native` remains beta and did not beat Yamux in the recorded comparison. |
| `shared.mux.native_initial_window_bytes` | Native mux window and Yamux maximum per-stream window | Despite the config name, the runtime maps this value to Yamux's maximum stream window too. Larger ceilings allow more in-flight data on high-BDP paths, with higher potential buffering per active stream. |
| `shared.tcp.send_buffer_bytes`, `recv_buffer_bytes` | Kernel TCP socket buffers | `0` leaves sizing to the OS. Set explicit sizes only after measuring the path and checking platform behavior. |
| `shared.pacing.burst_bytes`, `min_write_bytes`, `max_bytes_per_sec` | Application pacing burst, write granularity, and optional cap | Larger bursts can help bulk transfers but may increase queueing. `max_bytes_per_sec = 0` means uncapped. |
| `local.tunnel_pool.max_connections`, `interactive_lanes`, `bulk_lanes` | Number and preferred class of physical client tunnels | More lanes can improve concurrent workloads but add sockets, buffers, and server load. Lane counts must fit within the pool maximum. |
| `local.http_bulk_threshold_bytes` | HTTP upload classification threshold | Affects lane preference for qualifying HTTP requests; it does not classify HTTPS CONNECT, SOCKS5, or TUN flows as bulk. |
| `shared.underlay.http2.initial_stream_window_bytes`, `initial_connection_window_bytes` | HTTP/2 underlay flow-control windows | Tune only when using the HTTP/2 underlay. The connection window must be at least the stream window. |

The `auto-throughput` profile disables padding and timing jitter to favor
throughput. Do not use it when those traffic-shape properties are required.
Likewise, increasing `max_streams` or connection limits is capacity tuning,
not a throughput fix by itself; review memory and server resource limits.

## Yamux Window And Keepalive

For the byte accounting and update cycle shared by the mux implementations,
see [Stream Flow Control](STREAM-FLOW-CONTROL.md).

The production Yamux adapter starts each stream with the protocol's 256 KiB
window and permits the stream window to grow up to
`shared.mux.native_initial_window_bytes` (at least 256 KiB). This setting name
is historical: it is also passed to Yamux as `max_stream_window_size`. The
default value is 8 MiB. The shared `shared.max_streams` limit is passed to
Yamux as its stream-count cap; its default is 256. Yamux's own default count
and keepalive values therefore do not describe Espejismo's effective stream
limit.

Increasing the maximum window can help one long-lived stream keep a high
bandwidth-delay product path busy after the window becomes limiting. It does
not enlarge the initial 256 KiB, increase TCP capacity, or guarantee higher
throughput. A larger ceiling also raises possible buffering pressure when
many streams are active. Start with the profile-provided floor (16 MiB for
`auto-throughput`) on a measured high-RTT path, then change it only if
single-stream results point to flow control and memory headroom is known.
Coordinate this shared value across peers and benchmark with the same workload.

Yamux keepalive is enabled with a 30-second interval in the bundled
`tokio-yamux` default config; Espejismo currently inherits that setting. It is
separate from `shared.tcp.heartbeat_secs`, which sends Espejismo's encrypted
heartbeat. Keepalive detects/maintains the mux session; it is not a window
tuning knob or a substitute for the encrypted heartbeat. There is no public
TOML override for Yamux keepalive interval in the current runtime adapter.

## Benchmark And Record Results

Use [`BENCHMARKS.md`](../testing/BENCHMARKS.md) for harness setup and result
interpretation. `scripts/bench-throughput.sh` runs direct and proxied single
and parallel downloads/uploads in the same measurement window. Prefer several
rounds, identical endpoints and transfer sizes, and the median. Record the
profile, chunk settings, lane counts, RTT, host limits, logging level, and
environment with results. Avoid concurrent tunnel traffic when interpreting
admin byte counters. Do not compare runs with failed cases or materially
different path conditions.

Existing measurements in
[`THROUGHPUT_TUNING_HK2_RK.md`](../testing/THROUGHPUT_TUNING_HK2_RK.md)
show the variability involved: one five-round HK2-to-RK run measured median
four-way upload at 466.8 Mbit/s direct and 459.6 Mbit/s through Espejismo
(101% same-window efficiency), while other runs on the same general path
varied substantially. These are observations for that path and setup, not a
promise of expected speed on other links.

For tuning rationale and benchmark methodology from related open-source
projects, see [`REFERENCES.md`](../research/REFERENCES.md). The transferable
lesson is to measure on representative paths and control the benchmark
conditions; Espejismo retains its own TCP/Yamux design and product positioning.
