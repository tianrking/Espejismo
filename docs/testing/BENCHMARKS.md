# Espejismo Throughput Benchmarks (v0.1.5)

## Running the throughput harness

`scripts/bench-throughput.sh` compares direct HTTP transfers with the same
transfers through the local Espejismo HTTP proxy. Run it from a Linux test
client with `curl`, `awk`, `dd`, and `stat`; Python 3 is optional but required
for aggregate statistics. Start `espejismo-bench-http` on the remote test host
and ensure the local client and tunnel are already running. Use an isolated,
temporary benchmark port and test credentials, not production service settings.

For example, replace `<server-public-ip>` with the HTTP benchmark server's
reachable address:

```bash
espejismo-bench-http --listen 0.0.0.0:18082

ESPEJISMO_PROXY_URL=http://127.0.0.1:16681 \
ESPEJISMO_DIRECT_DOWNLOAD_URL=http://<server-public-ip>:18082/256m.bin \
ESPEJISMO_PROXY_DOWNLOAD_URL=http://127.0.0.1:18082/256m.bin \
ESPEJISMO_DIRECT_UPLOAD_URL=http://<server-public-ip>:18082/upload \
ESPEJISMO_PROXY_UPLOAD_URL=http://127.0.0.1:18082/upload \
ESPEJISMO_PARALLEL=4 \
ESPEJISMO_ROUNDS=5 \
scripts/bench-throughput.sh
```

The remote benchmark server must be reachable directly from the client for
the baseline URLs and through the tunnel for the proxy URLs. The harness runs
single-stream and parallel download/upload cases, with direct and proxy cases
in each round. Set `ESPEJISMO_ADMIN_URL` and (if configured) `ESPEJISMO_ADMIN_TOKEN`
to capture tunnel counter snapshots around each case. Keep the local process
log at the default `/tmp/espejismo-local-bench.log`, or set
`ESPEJISMO_LOCAL_LOG_FILE` to its actual path. The default log-risk gate stops
the run when recent logs contain oversized lines or mux frame-body dumps;
fix the logging level before collecting tuning data. The override
`ESPEJISMO_ALLOW_VERBOSE_LOGS=1` is for debugging only, since verbose logs can
distort throughput.

Useful controls include `ESPEJISMO_UPLOAD_FILE`, `ESPEJISMO_UPLOAD_MIB`,
`ESPEJISMO_PARALLEL`, `ESPEJISMO_ROUNDS`, `ESPEJISMO_ROUND_DELAY_SECS`,
`ESPEJISMO_CURL_MAX_TIME`, and `ESPEJISMO_OUTPUT_DIR`. Each run gets a unique
UTC run ID under `bench-results/` by default; set `ESPEJISMO_RUN_ID` to choose
one explicitly. The script refuses to overwrite an existing run directory.
See the variable defaults and validation in the script before changing other
`ESPEJISMO_*` settings.

## Reading a run

Each output directory contains `summary.md` (human-readable measurements),
`results.jsonl` (one machine-readable record per case), `environment.md`
(host/tool and run settings), `raw/` (curl output and errors), and
`log-risk.md`. When admin snapshots are enabled, it also contains
`admin-*.json`; the summary then includes a Tunnel Cost table.

Check every row's `OK` value first. A non-OK row or nonzero script exit means
the run is incomplete and its throughput should not be compared as a clean
result. Review `raw/` for curl status and errors. Compare median throughput
across several rounds, then use the per-case min/max and standard deviation to
judge variability; the script reports population standard deviation. Proxy
efficiency is the same-round proxy/direct throughput ratio, useful for
separating tunnel cost from path capacity. It is not a universal protocol
overhead figure: route changes, server load, CPU limits, and test ordering can
all affect it.

The Tunnel Cost table compares application bytes with local tunnel counter
deltas. It can expose framing, encryption, padding, control, and idle traffic,
but counter snapshots are process-wide and may include unrelated concurrent
traffic. Run with no other tunnel workload when interpreting those ratios.
Compare runs only when endpoints, transfer sizes, parallelism, profile,
logging level, and network conditions are materially alike. Record changes
and environment alongside results; do not infer an improvement from one noisy
round. These measurements characterize this tunnel and path, not a universal
speed ranking against other protocols.

## Methodology for comparable results

Treat a benchmark as a controlled comparison of one specific workload and
network path. Before collecting data, record the commit, client and server
hardware (including CPU limits), operating system, endpoint locations and
underlay, profile and effective settings, RTT, transfer size, parallelism,
round count, logging level, and any shaping or rate limits. Use the same
machines and server process for baseline and tunnel cases. Keep unrelated
traffic off the path where possible, and do not change settings between the
two arms of a comparison.

The harness runs direct and proxied cases in each round, which helps expose
short-term path variation. It does not currently perform a separate warm-up
phase or randomize case order. If startup, cache, or ordering effects could
matter, warm the endpoints consistently before the measured run and repeat
the comparison in a later run with the order reversed; describe these steps
alongside the results. Do not silently discard slow rounds. Investigate and
report failed transfers and unusual environmental events instead.

Use at least five successful rounds for a tuning comparison when practical.
Compare the median of each case, and report the observed range and standard
deviation to show spread. The script reports population standard deviation;
it does not calculate confidence intervals. For a formal uncertainty estimate,
collect more independent runs and calculate a stated confidence interval from
the per-round observations. Treat a small median difference as inconclusive
when it is comparable to the observed run-to-run variation; repeat under
controlled conditions before attributing it to a code or configuration
change.

For each round, compare proxy throughput with direct throughput for the same
direction, stream count, and transfer size. The resulting ratio describes
proxy efficiency on that path during that run. Also compare direct throughput
across runs: a changing baseline can indicate path or host variation rather
than a tunnel change. Report downloads and uploads separately, and keep
single-stream and parallel results distinct because they exercise different
limits. Throughput alone does not capture latency, CPU cost, memory use, or
traffic-shape behavior; include those measures when they are relevant to the
change being evaluated.

An improvement claim should identify the compared revisions/settings,
workload, median values, and spread, then state whether every measured case
completed successfully. A result is specific to its machines, path, and
conditions. Do not generalize it into a universal protocol ranking. This
measurement-first approach follows the benchmark practices called out in
[`REFERENCES.md`](../research/REFERENCES.md) (including iperf3/nuttcp's repeated
measurements), while evaluating Espejismo's own TCP/Yamux tunnel and preserving
the positioning in [`POSITIONING.md`](../POSITIONING.md).

Method: temporary `espejismo-remote` + `bench-http` on one side, temporary
`espejismo-local` on the other, downloading 64 MiB / uploading 32 MiB through
the tunnel with `--profile auto-throughput`. Temporary PSK, all instances
cleaned up afterwards. Servers: de (Germany), jp (Tokyo, 1 vCPU), rk (US),
gcp (GCP).

> The default profile only reaches ~12 KB/s on trans-Pacific links; all numbers
> below use `--profile auto-throughput`.

## Public IP underlay (2026-09-28)

Tunnel runs directly over public IPs; tailnet used for SSH management only.

| Pair | Download (MB/s) | Upload (MB/s) |
| --- | --- | --- |
| rk → de | 18.77 | 6.59 |
| rk → gcp | 4.13 | 7.86 |
| de → gcp | 1.52 | 1.94 |
| jp → rk | 2.70 | 0.78 |
| gcp → jp | 0.79 | 1.83 |
| de → jp | 0.33* | 0.55 |

\* de → jp download is unstable: the 64 MiB transfer broke around 28–53 MiB
(curl exit 18). Under investigation, see `docs/notes-long-transfer-stability.md`.

## Tailnet underlay (2026-09-28)

Same method, tunnel over Tailscale (WireGuard userspace) instead of public IP.

| Pair | Download (MB/s) | Upload (MB/s) |
| --- | --- | --- |
| rk ↔ de | 13.58 | 5.88 |
| rk ↔ gcp | 5.74 | 7.32 |
| de ↔ gcp | 1.31 | 2.57 |
| jp → rk | 2.41 | 0.79 |
| gcp → jp | 0.92 | 1.75 |
| de → jp | 0.40* | 0.97 |

## Takeaways

- rk ↔ de is fastest over public IP (18.77 vs 13.58 MB/s): no WireGuard
  userspace overhead.
- de ↔ jp is the outlier: slower and unstable over public IP (bad
  trans-Pacific routing); the jp box is also the weakest (1 vCPU Tokyo).
- All other pairs are roughly at parity between underlays; differences are
  within measurement noise.
