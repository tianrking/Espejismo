# Traffic Priority and QoS

Espejismo has two stream priority classes: `interactive` and `bulk`. Priority
helps separate short proxy exchanges from transfers that can occupy a tunnel
for longer. It is a scheduling hint inside the encrypted tunnel, not a network
wide QoS guarantee. It does not set IP DSCP, reserve bandwidth, cap rates, or
change the remote egress policy. For rate caps, see the pacing and per-user
bandwidth settings in [Configuration](CONFIG.md).

## What gets each priority

The client assigns priorities automatically:

| Traffic | Priority |
| --- | --- |
| SOCKS5 TCP and UDP | Interactive |
| HTTP `CONNECT` and ordinary requests | Interactive |
| HTTP request with `Content-Length` at or above `local.http_bulk_threshold_bytes` | Bulk |
| Plain HTTP `GET` path ending in a recognized archive, installer, image, package, or media suffix | Bulk |
| TUN TCP and UDP flows | Interactive |

The default HTTP upload threshold is `1048576` bytes (1 MiB). Set it to `0` to
disable size-based upload classification. The download-path rule remains
active. It checks the path suffix case-insensitively and ignores query and
fragment text. It is a heuristic: downloads with unrecognized or extensionless
paths remain interactive, and a qualifying `GET` is classified by path rather
than measured response size. See [HTTP Proxy Ingress](HTTP.md) for request
handling details.

There is no public setting to assign arbitrary priorities to destinations,
ports, or applications. In particular, TUN flows are currently interactive
regardless of transfer size.

## Configure preferred lanes

`local.tunnel_pool` controls how many physical client tunnels are dedicated to
each preferred class. The defaults are one interactive lane and two bulk
lanes, with a maximum pool size of four connections:

```toml
[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 1
bulk_lanes = 2
```

The lane counts must add up to no more than `max_connections`; at least one
lane is required. New streams prefer a lane of their class, then choose among
eligible lanes using current load and health information. If no lane of the
requested class is available, selection can fall back to another lane. A lane
preference therefore does not guarantee isolation or strict latency priority.
The client pool settings belong in the local configuration; the remote does
not need matching lane counts.

For a mixed browser workload, keep at least one interactive lane. Transfer-
heavy concurrent workloads can use more bulk lanes; workloads dominated by
short SOCKS5 or TUN exchanges can use more interactive lanes. Raising lane
counts consumes additional sockets, buffers, and remote connection capacity,
and may help concurrent flows without speeding up a single flow. Keep the
total within `max_connections` and measure under the actual workload. See
[Performance Tuning](PERFORMANCE.md) for profile trade-offs and benchmark
guidance.

## Multiplexer behavior

Priority is carried with tunnel stream requests. Lane preference is available
with the client's tunnel pool. In native mux mode, queued control frames are
sent first, followed by interactive data and then bulk data. Yamux remains the
recommended production mux; its priority effect is primarily through lane
selection, and neither mode provides preemption of data already written to the
underlying TCP connection. Do not treat these classes as a hard QoS or
bandwidth guarantee.
