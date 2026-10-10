# Resource planning

This page describes where Espejismo uses memory, CPU, and file descriptors and
how to size a small deployment. The figures below are configuration-derived
buffer budgets, not process RSS guarantees: allocator metadata, task state,
cryptographic state, kernel socket buffers, TLS/HTTP underlays, and application
traffic add variable overhead. Measure the target build and workload before
choosing production limits.

## Memory

The main tunable memory terms are:

| Resource | Default | Planning interpretation |
| --- | ---: | --- |
| `shared.tunnel_buffer` | 1 MiB | Frame transport duplex capacity per physical lane, on each process. The configured capacity is a per-lane budget, not a global pool. |
| Client `local.tunnel_pool.max_connections` | 4 | At the default buffer, up to about 4 MiB of tunnel duplex capacity per client process. |
| `shared.max_streams` | 256 | Global concurrent logical-stream permit count on the server. Each stream adds task/relay state and socket buffers, which vary with traffic and OS settings. |
| `remote.tarpit_max` | 1024 | Up to 1024 quarantined inbound TCP sockets when tarpit behavior is enabled. These consume descriptors and kernel memory. |
| `shared.mux.native_stream_buffer_frames` | 128 frames | Native mux inbound queue capacity per stream; frame payloads can be large, so this is not a small fixed byte allocation. |
| `shared.mux.native_send_queue_frames` | 64 frames | Native mux send queue capacity per session. |

The client default tunnel buffer budget scales approximately as
`max_connections × tunnel_buffer` (4 MiB at defaults). A server's corresponding
budget scales with active physical connections: `active_lanes × tunnel_buffer`.
The server's default physical connection ceiling is 1024, so the theoretical
duplex-capacity budget at the default buffer is 1 GiB if every slot is active.
This is a ceiling calculation, not a sensible default host reservation; set
`max_physical_connections` to the actual expected concurrency and available
memory.

For native mux, the configured initial window is 8 MiB per stream by default.
It is flow-control credit, not eagerly allocated memory, and must not be
multiplied into a guaranteed resident-memory figure. Queue occupancy and frame
sizes depend on traffic. The ordinary Yamux path also allocates per-stream
state and buffers; use observed RSS under representative concurrent transfers
to establish a safe `max_streams` value.

Stealth frame buffers can be as large as 64 KiB each. Larger frames and more
lanes increase transient framing memory. Adaptive throughput may raise the
tunnel buffer to a maximum of 32 MiB when its automatic tuning is eligible;
operator overrides can change which fields are eligible. Include this ceiling
when sizing high-RTT clients or servers.

## CPU

There is no fixed CPU reservation or worker-per-connection model. Work scales
with active traffic and includes XChaCha20-Poly1305 per frame, mux scheduling,
copying, optional padding/shaping, and underlay processing. Handshakes also use
X25519/HKDF and a configurable proof-of-work puzzle; the default puzzle is 12
bits. Unauthenticated puzzle verification is deliberately part of the
anti-probing path, so a burst of handshake attempts can use CPU even before a
session is established.

For capacity planning, measure CPU at the expected concurrent streams and
throughput, and separately observe handshake bursts. Reduce unnecessary lane,
stream, and handshake-puzzle settings only with security and latency needs in
mind. Espejismo does not disguise traffic as another protocol; resource tuning
does not change that behavior or its small-operations model.

## File descriptors

On the server, each active physical tunnel holds one accepted underlay socket.
Each active TCP relay normally adds one outbound destination socket; chained
egress can add further sockets. Therefore a useful baseline is:

```text
server descriptors ≈ active physical connections
                   + active TCP streams
                   + listeners/admin/metrics sockets
                   + quarantined tarpit sockets
                   + transient handshake and egress sockets
```

UDP relay paths use UDP sockets and may also retain a TCP control stream.
`shared.max_physical_connections` defaults to 1024 and `shared.max_streams` to
256 globally; `remote.tarpit_max` defaults to 1024. These limits are independent,
so their sum plus listeners is a conservative descriptor-planning baseline,
not a claim that every configuration reaches it. Ensure the service's OS file
descriptor limit exceeds expected concurrent use with operational headroom.

The client holds up to `local.tunnel_pool.max_connections` physical underlay
sockets (default 4), plus local SOCKS/HTTP listeners, accepted application
sockets, and transient destination-side proxy connections. TUN mode and
WebSocket/HTTP/2 underlays can add sockets or per-flow state. Plan descriptors
from expected local concurrency rather than only the lane count.

## Practical sizing

Use this sequence for each process independently; the client and server have
different lane, relay, and listener counts:

1. Choose the expected peak active physical lanes, active streams, and
   quarantined tarpit sockets. Use configured limits as ceilings only when
   planning for that full load; they are independent and need not all be
   reached together.
2. Calculate the explicitly sized tunnel-buffer budget as
   `active_lanes × effective_tunnel_buffer`. On a client, use the active pool
   size; on a server, use active accepted lanes. Resolve the effective buffer
   after profile overlays and adaptive throughput sizing, not just the TOML
   base value.
3. Measure a representative process baseline, then measure RSS at several
   controlled stream counts and lane counts under the intended workload. Use
   the observed increase per additional active stream as a planning estimate,
   not a universal constant. Include the mux mode, traffic mix, underlay,
   kernel socket buffer policy, and tarpit occupancy used for the measurement.
4. Form a working estimate from baseline RSS plus the measured RSS change at
   the target workload. Use the tunnel-buffer budget to explain/check that
   change, not as an extra term when it is already included in the measurement.
   Check queue occupancy and frame sizes for native mux separately; frame-count
   limits do not imply a fixed byte allocation. Do not add the mux flow-control
   window as allocated memory.
5. Add operational headroom for workload variation and bursts, then confirm
   RSS, CPU, open descriptors, and active lane/stream metrics during a
   representative peak. Set descriptor limits from concurrent lanes, relays,
   listeners, and tarpit allowance, with OS-level headroom. Raise application
   limits only when the measured host has room.

For example, four active client lanes with the default 1 MiB buffer correspond
to about 4 MiB of configured tunnel-buffer capacity. This is only one term in
the process budget: it excludes stream/task state, queues, allocator overhead,
and kernel memory, and it is not a prediction of RSS. Repeat the estimate with
the effective buffer if a profile or adaptive sizing raises it.

Configuration defaults and profile overlays are documented in
[`CONFIG.md`](CONFIG.md) and [`PROFILES.md`](PROFILES.md).
