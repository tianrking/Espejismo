# Metrics reference audit

## Audit and changes

Compared `docs/deployment/METRICS.md` with the shared collector and exporter in
`crates/espejismo-core/src/metrics.rs` and `crates/espejismo-core/src/admin.rs`,
then checked metric update sites in the local and remote handlers and tunnel
manager. The catalog, names, types, labels, directionality, and bounds were
otherwise consistent with the implementation.

Clarified three scope details in the reference: process counters and lane
counters have different reset lifetimes (lane counters reset when a local
tunnel manager is replaced); accepted connections are counted only after the
remote global connection limit, and local accepts refer to SOCKS5/HTTP proxy
listeners; and remote active-stream gauges only count streams admitted past the
per-user limit. The adaptive score is an internal ranking value without a
latency or throughput unit. Lane samples can also be absent before the runtime
publishes a lane.

No runtime or positioning changes. Expected benefit: Prometheus users can
interpret resets, accepted-connection totals, and active-stream values against
the actual event points. No performance or resource improvement is expected.

## Verification

- `cargo test --offline -p espejismo-core metrics::tests`: 5 passed, 0 failed.
- `cargo test --offline -p espejismo-core admin::tests::runtime_prometheus_includes_lane_counters`: 1 passed, 0 failed.
- No performance experiment applies to this documentation-only audit.
