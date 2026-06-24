# HK2 to RK Throughput Tuning Report

Date: 2026-06-24

This report records the live throughput tuning pass between the HK2 client host
and the RK server host. The goal was to improve TCP tunnel efficiency on a
high-latency and lossy path without changing the censorship-resistance defaults
used by stealth mode.

## Test Topology

```text
HK2 client host
  espejismo-local
  HTTP proxy: 127.0.0.1:16681
        |
        | Espejismo TCP + Yamux tunnel
        v
RK server host
  espejismo-remote: 0.0.0.0:16690
  HTTP download source: 0.0.0.0:18082
  HTTP upload sink: 0.0.0.0:18083
```

The test port set was temporary and separate from the long-running production
service ports.

## Reproducible Benchmark Harness

Use `scripts/bench-throughput.sh` from HK2 to capture direct-link and
Espejismo-link performance in the same window:

```bash
ESPEJISMO_PROXY_URL=http://127.0.0.1:16681 \
ESPEJISMO_DIRECT_DOWNLOAD_URL=http://<rk-public-ip>:18082/256m.bin \
ESPEJISMO_PROXY_DOWNLOAD_URL=http://127.0.0.1:18082/256m.bin \
ESPEJISMO_DIRECT_UPLOAD_URL=http://<rk-public-ip>:18083/upload \
ESPEJISMO_PROXY_UPLOAD_URL=http://127.0.0.1:18083/upload \
ESPEJISMO_UPLOAD_FILE=/tmp/espejismo-upload-128m.bin \
ESPEJISMO_PARALLEL=4 \
ESPEJISMO_ADMIN_URL=http://127.0.0.1:9090/status \
ESPEJISMO_ADMIN_TOKEN=change-me-admin-token \
scripts/bench-throughput.sh
```

The harness records:

- direct download and upload, single stream and parallel streams;
- proxied download and upload, single stream and parallel streams;
- raw curl timings and byte counts;
- `results.jsonl` for later comparison;
- `summary.md` for human review;
- optional admin snapshots before and after the run.

This is the right way to compare future tuning changes because the direct tests
and proxied tests run in the same measurement window.

## Baseline Link

The physical path is asymmetric and noisy:

| Test | Direction | Result |
| --- | --- | ---: |
| Ping | HK2 to RK | about 156 ms RTT, 0% loss during sample |
| iperf3 P1 | HK2 to RK | about 657 Mbit/s receiver |
| iperf3 P4 | HK2 to RK | about 793 Mbit/s receiver |
| iperf3 P8 | HK2 to RK | about 814 Mbit/s receiver |
| iperf3 reverse P4 | RK to HK2 | about 87 Mbit/s receiver |

The download direction tested through Espejismo is constrained by the RK to HK2
path, so it is expected to top out near 85 to 90 Mbit/s regardless of local
optimizations.

## Implemented Changes

### 1. Larger Bulk-Capable Normal Frames

Normal non-stealth encrypted frames were raised from a 64 KiB class limit to a
256 KiB class limit. The effective payload cap is 262127 bytes after frame type
and AEAD tag overhead.

Stealth frames remain capped at 64 KiB and should stay small in production
cover-traffic deployments.

### 2. Configurable Bulk Chunk Ceiling

`shared.obfuscation.max_chunk` now acts as the operator-selected bulk ceiling.
The recommended high-BDP values are:

```toml
[shared.obfuscation]
profile = "bulk"
chunk_policy = "bulk"
randomize_chunks = false
min_chunk = 65536
max_chunk = 262127
```

Use `131072` as a conservative middle value on paths where 256 KiB bursts make
loss recovery or queueing worse.

### 3. Native Mux Payload Cap Alignment

The in-tree native mux frame payload cap was raised to 256 KiB so it remains
compatible with the larger encrypted-frame path during tests and benchmarks.

Yamux remains the recommended production mux mode.

### 4. HTTP Large-Request Bulk Classification

`local.http_bulk_threshold_bytes` was added. HTTP proxy requests with
`Content-Length` greater than or equal to this value open the tunnel stream with
bulk priority.

Default:

```toml
[local]
http_bulk_threshold_bytes = 1048576
```

Set it to `0` to keep all HTTP proxy streams on interactive lanes.

### 5. Strict Preferred-Lane Selection

Lane selection now first chooses from the preferred lane class and only falls
back to the opposite class if no preferred lane exists. This keeps small
interactive flows on interactive lanes and large classified flows on bulk lanes.

## Recommended High-Throughput Baseline

Use this on both peers where applicable:

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
mode = "yamux"
native_initial_window_bytes = 8388608

[local]
http_bulk_threshold_bytes = 1048576

[local.tunnel_pool]
min_connections = 1
max_connections = 8
interactive_lanes = 1
bulk_lanes = 4
```

For ordinary browser use, keep more interactive lanes:

```toml
[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 2
bulk_lanes = 2
```

## Measurements

### Pre-Change Baseline

| Test | Direct | Espejismo | Efficiency |
| --- | ---: | ---: | ---: |
| Download 256 MiB, single stream | 86.0 Mbit/s | 85.5 Mbit/s | 99% |
| Upload 256 MiB, single stream | 341 Mbit/s | 244 Mbit/s | 72% |
| Upload 4 x 128 MiB | 517.5 Mbit/s | 372.4 Mbit/s | 72% |

Observation: download proxy overhead was already low. Upload was the main
efficiency gap.

### Larger-Frame Runs

| Test | Result |
| --- | ---: |
| Upload 256 MiB, single stream | 279 to 396 Mbit/s across three runs |
| Upload 4 x 128 MiB, best observed | 491.8 Mbit/s |
| Upload 4 x 128 MiB, 128 KiB chunk setting | 394.8 Mbit/s |
| Upload 4 x 128 MiB, later 262127-byte run | 378.7 Mbit/s |
| Direct upload 4 x 128 MiB, same later window | 473.9 Mbit/s |

The best observed proxy result improved from 372.4 Mbit/s to 491.8 Mbit/s,
about a 32% improvement over the initial Espejismo P4 baseline. Later runs were
lower, which matches the observed raw-link variance.

### HTTP Bulk Classification A/B

Both A/B runs used:

```toml
[local.tunnel_pool]
interactive_lanes = 1
bulk_lanes = 4
```

| Test | Setting | Result |
| --- | --- | ---: |
| Upload 4 x 128 MiB | `http_bulk_threshold_bytes = 0` | 387.6 Mbit/s |
| Upload 4 x 128 MiB | `http_bulk_threshold_bytes = 1048576` | 380.3 Mbit/s |

The classification feature is implemented and unit-tested, but this short live
A/B did not show a stable throughput gain. The likely causes are raw path
variance, the Python HTTP sink, and the need for structured per-lane runtime
metrics during the benchmark.

### Structured Harness Validation

After adding `scripts/bench-throughput.sh` and per-lane admin metrics, the same
HK2 to RK topology was retested with direct and proxied checks in one run.

| Commit | Direct P4 Upload | Proxy P4 Upload | Efficiency | Lane Finding |
| --- | ---: | ---: | ---: | --- |
| `c46db48` | 504.2 Mbit/s | 412.6 Mbit/s | 82% | Only two bulk lanes carried upload streams |
| `6245148` | 471.8 Mbit/s | 415.2 Mbit/s | 88% | Four bulk lanes were created, but latency score still biased one lane |
| `9fd9f94` | 538.3 Mbit/s | 456.0 Mbit/s | 85% | P4 upload was distributed across the four bulk lanes |

The `c46db48` run proved the benchmark harness was necessary: aggregate
throughput alone looked acceptable, but the lane snapshot showed the concurrent
open race clearly. `6245148` added pending-open reservations so simultaneous
streams could not all select the same lane before `active_streams` changed.
`9fd9f94` then made lane scoring load-first, with open latency only as a
tie-breaker, so an idle lane wins over a loaded low-latency lane.

Latest validation run:

| Test | Direct | Espejismo | Efficiency |
| --- | ---: | ---: | ---: |
| Download 256 MiB, single stream | 87.0 Mbit/s | 83.4 Mbit/s | 96% |
| Download 4 x 256 MiB | 91.6 Mbit/s | 89.8 Mbit/s | 98% |
| Upload 128 MiB, single stream | 240.9 Mbit/s | 130.0 Mbit/s | 54% |
| Upload 4 x 128 MiB | 538.3 Mbit/s | 456.0 Mbit/s | 85% |

Latest artifacts on HK2:

```text
/tmp/espejismo-bench-results/20260624T0321-idle-lane/summary.md
/tmp/espejismo-bench-results/20260624T0321-idle-lane/results.jsonl
/tmp/espejismo-bench-results/20260624T0321-idle-lane/admin-after.json
```

The remaining single-stream upload gap is real, but it is a different problem
from P4 lane scheduling. It likely combines one-stream flow-control limits,
frame/mux overhead, and the Python upload sink. The P4 case is now much closer
to the raw link and is the better model for browser/TUN workloads with multiple
flows.

## What Changed in Efficiency

| Area | Before | After |
| --- | --- | --- |
| Bulk frame payload | Up to about 64 KiB | Operator-selectable up to 262127 bytes |
| Bulk chunk config | Larger configured values were clamped to 64 KiB | `max_chunk` can tune 64 KiB, 128 KiB, or 256 KiB-class frames |
| HTTP upload lane priority | Always interactive | Large `Content-Length` requests can use bulk lanes |
| Lane selection | Preferred lanes could still lose to lower-score opposite lanes | Preferred lane class is tried first |
| Best observed proxy P4 upload | 372.4 Mbit/s | 491.8 Mbit/s |

## Conclusions

The frame-size change is the most useful improvement from this pass. It reduced
per-frame overhead and produced the best observed proxy upload throughput.

The HTTP bulk lane feature is architecturally correct and useful for future
scheduling, but this live test did not prove a separate speedup from lane
classification alone.

Native mux did not outperform Yamux in this run:

| Mux | Upload 4 x 128 MiB |
| --- | ---: |
| Yamux, best large-frame run | 491.8 Mbit/s |
| Native mux comparison | 384.2 Mbit/s |

Keep `shared.mux.mode = "yamux"` for production.

## Follow-Up Work

- Add a non-Python test sink such as `iperf3`-style HTTP body discard or a small
  Rust sink to remove Python server overhead from upload tests.
- Test under controlled loss and latency using Linux `tc netem`.
- Add repeated-run aggregation to the benchmark harness so it can report median,
  min, max, and standard deviation across several adjacent windows.
- Keep stealth deployments on smaller fixed-size frames; use large bulk frames
  only for throughput-oriented profiles.
