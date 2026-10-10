# CPU and Memory Profiling

This guide helps contributors locate CPU hot spots and memory growth in the
Linux client or server. Profile a representative workload with the same
configuration and limits as the behavior being investigated; profiles explain
where resources go, while benchmarks establish whether a change improved
performance.

## Prepare a symbolized release build

The workspace release profile strips symbols. Override that for a local
profiling build so sampled stacks can be attributed to Rust functions:

```bash
RUSTFLAGS="-C debuginfo=1 -C strip=none" cargo build --release -p espejismo-client
RUSTFLAGS="-C debuginfo=1 -C strip=none" cargo build --release -p espejismo-server
```

Use the exact binary produced by the selected package and keep the build
configuration fixed across comparisons. Profiling instrumentation can change
timing and memory use; compare profiles qualitatively, then confirm any
optimization with the normal benchmark procedure in
[`BENCHMARKS.md`](../testing/BENCHMARKS.md).

## CPU samples with Linux perf

Install the distribution's `perf` package and ensure kernel policy permits
sampling. Start the process under its normal workload, then record a bounded
interval (replace the PID and output path):

```bash
sudo perf record -F 99 -g -p <pid> -o /tmp/espejismo-cpu.data -- sleep 30
perf report -i /tmp/espejismo-cpu.data
```

For a flame graph, use Brendan Gregg's FlameGraph scripts if installed:

```bash
perf script -i /tmp/espejismo-cpu.data \
  | stackcollapse-perf.pl \
  | flamegraph.pl > /tmp/espejismo-cpu.svg
```

The graph shows sampled on-CPU stacks, not wall-clock time spent waiting on
network I/O. Capture client and server separately when both may be limiting.
Record CPU model, process arguments, workload, profile, duration, and whether
the process was CPU-limited.

## Memory growth

First observe the running process under a fixed workload. Linux `smaps_rollup`
provides a useful resident-set snapshot; take repeated samples rather than
interpreting one peak:

```bash
while kill -0 <pid> 2>/dev/null; do
  date -Is
  awk '/^(Rss|Pss|Private_Clean|Private_Dirty):/ {print}' \
    /proc/<pid>/smaps_rollup
  sleep 5
done
```

For allocation call stacks, install Heaptrack and run the symbolized binary
under it (example arguments/configuration are application-specific):

```bash
heaptrack target/release/espejismo-local --config /path/to/client.toml
heaptrack_gui heaptrack.espejismo-local.*.gz
```

Heaptrack records allocator activity and adds overhead. It does not account
for every source of process memory, such as kernel socket buffers or mappings;
use RSS/PSS observations alongside allocation reports. Valgrind Massif is an
alternative heap profiler where available, with substantial runtime overhead.

## Compare and report

Use identical transfer sizes, concurrency, endpoints, profile settings, and
runtime duration for baseline and candidate. Include idle/startup measurements
when investigating retained memory, and include sustained load when examining
buffer growth. Report peak and end-of-run RSS/PSS, allocation hot spots, CPU
sample shares, tool versions, and any workload failures. Repeat noisy runs.
Do not infer throughput gains from a flame graph alone; use the repeated
throughput method in [`BENCHMARKS.md`](../testing/BENCHMARKS.md).

These tools diagnose the existing authenticated encrypted TCP/Yamux tunnel.
Profiling does not imply protocol camouflage or a change to Espejismo's
positioning; see [`POSITIONING.md`](../POSITIONING.md).
