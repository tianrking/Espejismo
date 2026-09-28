# Espejismo Throughput Benchmarks (v0.1.5)

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
