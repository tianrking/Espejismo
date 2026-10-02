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
