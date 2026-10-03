# Performance Documentation Index

Use this page to find Espejismo's performance guidance and measured results.
Measurements describe their recorded hosts, route, configuration, and workload;
they are not speed guarantees for other paths. Espejismo remains a native
authenticated encrypted TCP/Yamux tunnel and does not use protocol camouflage.

## Tune a deployment

- [Performance tuning guide](../deployment/PERFORMANCE.md) — select a profile,
  identify likely bottlenecks, and understand setting trade-offs.
- [Profiles](../deployment/PROFILES.md) — configuration overlays and their
  intended workloads.
- [Stream flow control](../deployment/STREAM-FLOW-CONTROL.md) — mux window
  accounting and limits.
- [Resource planning](../deployment/RESOURCES.md) — memory and connection
  budgets relevant to larger buffers and more lanes.
- [Traffic shaping and obfuscation](../deployment/OBFUSCATION.md) — packet
  shape features and their throughput costs.
- [Known issues](../KNOWN_ISSUES.md) — current performance-related limitations.

## Measure and interpret

- [Benchmark instructions and methodology](BENCHMARKS.md) — run the throughput
  harness, compare repeated rounds, and record environmental conditions.
- [`bench-throughput.sh`](../../scripts/bench-throughput.sh) — direct and
  proxied download/upload comparison harness.

## Recorded results

- [HK2 to RK throughput tuning](THROUGHPUT_TUNING_HK2_RK.md) — multi-round
  tuning history, A/B comparisons, and path variability.
- [v0.1.3 HK2/RK mode matrix](V0.1.3_HK2_RK_MODE_MATRIX.md) — recorded
  throughput and efficiency across transport modes and profiles.

## Research basis

- [Reference projects and benchmark methods](../research/REFERENCES.md) —
  transferable implementation and measurement ideas from Hysteria2, quic-go,
  yamux, and other projects, with positioning boundaries.
- [Project positioning](../POSITIONING.md) — the constraints tuning choices
  must preserve.

Follow the documented measurement discipline: compare the same workload on
the same path, review successful rounds and spread, and make one tuning change
at a time. Do not attribute path variation to a setting without repeatable
data.
