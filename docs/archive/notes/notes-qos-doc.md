# Traffic priority documentation

## Findings and plan

The runtime has two stream classes (`Interactive` and `Bulk`) carried in
tunnel requests. The local HTTP proxy classifies requests with a configurable
1 MiB default content-length threshold and a fixed suffix heuristic for plain
HTTP `GET` downloads. SOCKS5 TCP/UDP and TUN TCP/UDP currently use interactive
priority. The client pool defaults to one interactive and two bulk lanes
(maximum four connections), prefers the requested class, and scores lanes by
load and health with fallback when necessary. Native mux drains control,
interactive, then bulk frames; this is not a network-wide QoS mechanism.

The existing HTTP guide covered HTTP-specific classification, but there was no
single explanation of the priority classes and lane-count trade-offs. Added
`docs/deployment/QOS.md` and linked it from the configuration guide. It
documents the implemented classifications, threshold behavior, suffix
heuristic, lane configuration and constraints, fallback, resource costs, and
the limits of the scheduling guarantee. This is strictly descriptive: no
classification, protocol behavior, or product positioning changes. The
references list (`docs/research/REFERENCES.md`) and positioning
(`docs/POSITIONING.md`) were reviewed; related projects emphasize workload-aware
scheduling and measurement, while Espejismo keeps its native authenticated,
encrypted tunnel and small operations model.

Expected performance change: 0%; documentation only. The intended benefit is
lower operator confusion and more accurate lane sizing, not a runtime speedup.

## Verification

Compared the description with `StreamPriority`, HTTP classification,
`local.tunnel_pool` defaults and validation, lane selection/scoring, and the
native pending-frame queue. Reviewed the Markdown links and TOML example for
consistency. No Rust source changed, so no cargo tests or throughput benchmark
were run. Conclusion: docs-only change; no runtime regression introduced.
