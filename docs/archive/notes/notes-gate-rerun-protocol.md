# Gate: Rerun Six Protocol Benchmark Pairs

Date: 2026-09-29

## Scope and method

This task is a measurement rerun, not a protocol or implementation change. The
existing v0.1.5 benchmark record in [`BENCHMARKS.md`](../../../docs/testing/BENCHMARKS.md)
defines six server pairs and compares the same `auto-throughput` tunnel method
over public IP and tailnet. The fair comparison is to run both underlays for
each pair with the current `main` binaries, identical 64 MiB download and 32
MiB upload payloads, configuration, and direction, and record both outcomes
side by side. This keeps link variation from being mistaken for a protocol
gain. The throughput harness guidance also recommends adjacent measurements,
medians, and capturing environment metadata; those are preferred where the
existing six-pair setup permits them.

The performance-testing guidance in [`REFERENCES.md`](../../../docs/research/REFERENCES.md)
points to iperf3/nuttcp and multiple rounds with medians and confidence
intervals. For this product, the existing paired same-payload tunnel method is
more directly relevant than introducing a different transport benchmark. The
comparison does not alter Espejismo's TCP tunnel, encrypted-chaos behavior, or
minimal operations model described in [`POSITIONING.md`](../../../docs/POSITIONING.md).

## Expected value

No performance gain is being proposed: the goal is fresh, fair current-main
data across six endpoint pairs, including the public-IP versus tailnet
difference. This can establish whether the existing throughput conclusions
still hold after recent changes and distinguish path noise from tunnel
behavior. No quantitative gain is claimed in advance.

## Execution findings

The checkout is on `main` at `8899d49` (`Raise yamux stream window default to
8 MiB; BDP-driven per-session window`), and the working tree was clean before
this note. This execution environment is the RK host (`racknerd-fce354b`). The
documented live HK2/RK benchmark hostname `rk.w0x7ce.eu` did not resolve from
the sandbox (`curl` returned exit 6, `Could not resolve host`). No benchmark
server, local proxy, or configured six-pair runner is available in this
checkout environment; process inspection found no benchmark/tunnel process.
The network namespace also denies `ss` netlink inspection.

Therefore no valid throughput samples were collected. Existing numbers in
`testing/BENCHMARKS.md` are historical and must not be relabeled as a rerun.
The hard performance gate is unmet, so there is no improvement/no-regression
conclusion and the root `.commit-msg` must not be refreshed for this task.

## Pending rerun

Run from an environment with access to all six endpoint pairs, using current
`main` binaries on both sides, the same temporary benchmark service and
payload sizes for each underlay, and paired public-IP/tailnet observations.
Record timestamp, binary revision, direction, RTT, payload sizes, rounds and
per-run values; report medians and failures. Keep failed/incomplete transfers
visible rather than excluding them. Then update `testing/BENCHMARKS.md` with
the six pair results and summarize the measured deltas here.
