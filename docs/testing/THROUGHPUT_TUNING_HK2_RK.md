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
  Rust benchmark HTTP source/sink: 0.0.0.0:18082
```

The test port set was temporary and separate from the long-running production
service ports.

## Reproducible Benchmark Harness

Use `scripts/bench-throughput.sh` from HK2 to capture direct-link and
Espejismo-link performance in the same window:

```bash
espejismo-bench-http --listen 0.0.0.0:18082

ESPEJISMO_PROXY_URL=http://127.0.0.1:16681 \
ESPEJISMO_DIRECT_DOWNLOAD_URL=http://<rk-public-ip>:18082/256m.bin \
ESPEJISMO_PROXY_DOWNLOAD_URL=http://127.0.0.1:18082/256m.bin \
ESPEJISMO_DIRECT_UPLOAD_URL=http://<rk-public-ip>:18082/upload \
ESPEJISMO_PROXY_UPLOAD_URL=http://127.0.0.1:18082/upload \
ESPEJISMO_UPLOAD_FILE=/tmp/espejismo-upload-128m.bin \
ESPEJISMO_PARALLEL=4 \
ESPEJISMO_ROUNDS=5 \
ESPEJISMO_ADMIN_URL=http://127.0.0.1:9090/status \
ESPEJISMO_ADMIN_TOKEN=change-me-admin-token \
scripts/bench-throughput.sh
```

The harness records:

- direct download and upload, single stream and parallel streams;
- proxied download and upload, single stream and parallel streams;
- raw curl timings and byte counts;
- `results.jsonl` for later comparison;
- `summary.md` with per-round and aggregate median/mean/min/max/stddev review;
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

### 4. HTTP Bulk Classification

`local.http_bulk_threshold_bytes` was added. HTTP proxy requests with
`Content-Length` greater than or equal to this value open the tunnel stream with
bulk priority. Plain HTTP `GET` requests with common large-download suffixes
(`.bin`, `.zip`, `.tar.gz`, `.mp4`, `.iso`, and similar archive/package/media
paths) also open bulk streams because downloads normally do not have a request
body `Content-Length`.

Default:

```toml
[local]
http_bulk_threshold_bytes = 1048576
```

Set it to `0` to disable the upload-size threshold. Filename-style plain HTTP
download classification still applies. HTTPS `CONNECT` streams remain
interactive until a future sniffing/routing layer can classify them from SNI or
policy rules.

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

### Adaptive Lane Scoring and Metered Lane Counters

Commit `16fc542` adds adaptive lane scoring from active streams, pending opens,
stream-open failures, last error state, open latency, RTT trend, and recent
EWMA throughput. It also measures lane bytes from actual tunnel stream
`poll_read` / `poll_write` calls, which fixes the earlier HTTP upload
observability gap where fixed-length request bodies were not visible in
per-lane counters.

HK2 to RK was retested with the Rust benchmark source/sink, `auto-throughput`,
one interactive lane, six bulk lanes, four-way parallelism, and five adjacent
rounds:

| Test | Direct Median | Espejismo Median | Median Efficiency | Notes |
| --- | ---: | ---: | ---: | --- |
| Download 256 MiB, single stream | 84.9 Mbit/s | 87.7 Mbit/s | 103% | Direction is capped by the RK to HK2 path and varies by window. |
| Download 4 x 256 MiB | 90.8 Mbit/s | 88.5 Mbit/s | 97% | One round dipped to 63.7 Mbit/s, so mean efficiency was lower. |
| Upload 128 MiB, single stream | 225.8 Mbit/s | 214.8 Mbit/s | 95% | Single-stream upload gap is now much smaller than the earlier 54% run. |
| Upload 4 x 128 MiB | 466.8 Mbit/s | 459.6 Mbit/s | 101% | Proxy P4 tracked raw-link variance closely and sometimes beat same-window direct. |

Aggregate spread from the five-round run:

| Test | Proxy Median | Proxy Min | Proxy Max | Stddev |
| --- | ---: | ---: | ---: | ---: |
| proxy-download-p1 | 87.7 Mbit/s | 79.1 | 88.2 | 3.5 |
| proxy-download-pN | 88.5 Mbit/s | 63.7 | 90.1 | 10.0 |
| proxy-upload-p1 | 214.8 Mbit/s | 164.3 | 215.5 | 20.1 |
| proxy-upload-pN | 459.6 Mbit/s | 434.8 | 475.7 | 14.0 |

The final admin snapshot showed the adaptive counters working on real traffic:

| Lane | Class | Streams | Client to Remote | Remote to Client | Recent C2R bps | Recent R2C bps | Score |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | interactive | 25 | 3,525 | 6,710,889,025 | 11 | 25,040,753 | 0 |
| 1 | bulk | 1 | 1,048,790 | 108 | 7,172,320 | 738 | 25,327 |
| 2 | bulk | 10 | 1,342,179,660 | 1,100 | 163,224,425 | 132 | 0 |
| 3 | bulk | 5 | 671,089,830 | 550 | 122,134,293 | 98 | 0 |
| 4 | bulk | 5 | 671,089,830 | 550 | 127,378,945 | 103 | 0 |
| 5 | bulk | 5 | 671,089,830 | 550 | 126,468,391 | 102 | 0 |

The `Tunnel Cost` table in the same run now reports upload primary tunnel bytes
at 100% of curl application upload bytes plus only the expected protocol
overhead. This proves the benchmark can separate raw-link movement from
Espejismo overhead instead of hiding upload bodies from lane accounting.

Latest artifacts on HK2:

```text
/tmp/espejismo-bench-results/20260624T0955-16fc542-adaptive-metered/summary.md
/tmp/espejismo-bench-results/20260624T0955-16fc542-adaptive-metered/results.jsonl
/tmp/espejismo-bench-results/20260624T0955-16fc542-adaptive-metered/admin-round-5-after.json
```

## What Changed in Efficiency

| Area | Before | After |
| --- | --- | --- |
| Bulk frame payload | Up to about 64 KiB | Operator-selectable up to 262127 bytes |
| Bulk chunk config | Larger configured values were clamped to 64 KiB | `max_chunk` can tune 64 KiB, 128 KiB, or 256 KiB-class frames |
| HTTP upload lane priority | Always interactive | Large `Content-Length` requests can use bulk lanes |
| HTTP plain download lane priority | Always interactive | Common large-download `GET` paths can use bulk lanes |
| Lane selection | Preferred lanes could still lose to lower-score opposite lanes | Preferred lane class is tried first |
| Lane score | Load and open latency only | Load, pending opens, failure ratio, errors, RTT trend, open latency, and recent throughput |
| Lane byte metrics | Fixed-length upload bodies could be invisible until helper return paths lined up | Actual tunnel stream reads/writes drive counters and throughput EWMA |
| Best observed proxy P4 upload | 372.4 Mbit/s | 491.8 Mbit/s |

## Conclusions

The frame-size change is the most useful improvement from this pass. It reduced
per-frame overhead and produced the best observed proxy upload throughput.

The HTTP bulk lane feature is architecturally correct and useful for future
scheduling, but this live test did not prove a separate speedup from lane
classification alone.

The first five-round adaptive benchmark also revealed a classification gap:
HTTP `/256m.bin` downloads have no request body `Content-Length`, so their
6.7 GiB response traffic stayed on the interactive lane while uploads were
correctly spread across bulk lanes. The post-`v0.1.4` HTTP path-suffix
classifier fixes that specific plain HTTP benchmark and release-download case;
HTTPS `CONNECT`, SOCKS5, and TUN traffic still need the planned routing/sniffing
layer for richer policy-based bulk detection.

Focused post-fix validation on `d00ed7f`:

```text
HK2 client build: /root/Espejismo at d00ed7f
RK remote service: espejismo-remote 0.1.4
Request: curl -x http://127.0.0.1:16681 http://rk.w0x7ce.eu:18082/1m.bin
Result: HTTP 200, 1,048,576 bytes, 615,824 B/s, 1.702719 s
RK relay log: target=rk.w0x7ce.eu:18082 priority=Bulk
Local admin metrics: stream_opened=1, stream_failed=0, active_streams=0
Bulk lane 1: streams=1, c2r=139, r2c=1,048,679, recent_r2c=6,212,990 bps
```

This proves the plain HTTP `.bin` classifier moves completed download response
bytes onto a bulk lane. A larger `/16m.bin` probe was intentionally stopped
after 60 seconds on this live path and exposed a separate observability gap:
older builds only committed per-lane byte counters to admin snapshots after the
stream completed. The follow-up live-counter change makes active stream bytes
visible in the lane snapshot as they move.

Native mux did not outperform Yamux in this run:

| Mux | Upload 4 x 128 MiB |
| --- | ---: |
| Yamux, best large-frame run | 491.8 Mbit/s |
| Native mux comparison | 384.2 Mbit/s |

Keep `shared.mux.mode = "yamux"` for production.

## Follow-Up Work

- Use the Rust `espejismo-bench-http` source/sink for future HK2/RK runs to
  remove Python server overhead from upload tests.
- Test under controlled loss and latency using Linux `tc netem`.
- Add explicit benchmark cases for SOCKS5 and TUN traffic classification, not
  only HTTP proxy uploads and downloads.
- Keep stealth deployments on smaller fixed-size frames; use large bulk frames
  only for throughput-oriented profiles.
