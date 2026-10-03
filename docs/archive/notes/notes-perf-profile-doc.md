# CPU and memory profiling documentation

## Findings and plan

Reviewed `docs/research/REFERENCES.md` and `docs/POSITIONING.md` first. The
reference list prioritizes controlled, repeated measurements (as in iperf3,
nuttcp, and the cited project benchmark practices). Existing Espejismo
performance docs cover throughput and configuration trade-offs, but did not
explain how contributors can locate CPU hot spots or track process memory.

Added a Linux-focused profiling guide covering symbolized release builds,
`perf` CPU sampling, Heaptrack allocator traces, and `/proc` RSS/PSS snapshots.
Linked it from the development and performance indexes. It distinguishes
sampled CPU from I/O wait, allocator data from total process memory, and
diagnostic profiles from benchmark evidence. No dependencies or runtime
behavior changed.

Expected runtime performance change: **0%**. This is documentation only; the
expected benefit is a repeatable contributor workflow for investigating
resource costs. No quantitative reduction in profiling time is claimed. The
guidance applies the reference projects' measurement discipline to the
existing TCP/Yamux architecture and preserves the non-camouflage positioning
and small operational model in `docs/POSITIONING.md`.

## Validation and evidence

Reviewed commands against the Linux `perf`, `/proc`, and Heaptrack workflows
and checked internal links. Throughput before/after comparison and Cargo tests
are not applicable: no executable, configuration, or benchmark behavior
changed, and no runtime gain is claimed. Therefore no experiment data or
improvement percentage is claimed for this documentation-only change.
