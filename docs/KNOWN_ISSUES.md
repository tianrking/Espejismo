# Known Issues

This page tracks confirmed or actively investigated limitations that can
affect current deployments.

## Long transfers on de → jp break mid-way (under investigation)

64 MiB downloads from de (Germany) to jp (Tokyo) intermittently break around
28–53 MiB (curl exit 18). Seen on both public-IP and tailnet underlays.
Smaller transfers and other server pairs are unaffected. Root cause not yet
confirmed — see `docs/notes-long-transfer-stability.md`.

## Default profile is slow on high-RTT links

With the default profile, trans-Pacific links only reach ~12 KB/s. Use
`--profile auto-throughput` (or the adaptive throughput helper, which applies
it automatically when RTT ≥ 100 ms). See `docs/testing/BENCHMARKS.md`.
