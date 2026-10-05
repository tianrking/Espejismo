# Local loopback baseline

## Change and rationale

Replaced the old yamux benchmark's fixed port, leaked global stream handle,
and `unwrap()`-on-EOF echo loop with two matched 512 KiB echo cases: raw TCP
and yamux. Each Criterion iteration opens a fresh connection, transfers one
fixed payload, reads the complete response, and checks byte equality. Both
listeners request an ephemeral loopback port, so concurrent runs do not depend
on port 12345 being free. The echo task treats a closed connection as normal
termination. This follows the repeatable warmup and comparison approach in
iperf3/nuttcp noted in `docs/research/REFERENCES.md`, while retaining the
project's native TCP/yamux transport and making no protocol or positioning
change.

Expected result: a repeatable raw TCP reference next to yamux overhead. No
throughput improvement is claimed; this harness establishes the baseline for
future comparisons.

## Verification

- `cargo bench -p tokio-yamux --bench bench --offline -- --sample-size 10 --measurement-time 1 --warm-up-time 1`: benchmark compiled successfully, then could not start because the sandbox denies loopback socket bind (`PermissionDenied: Operation not permitted`). No performance numbers were produced.
- `cargo test -p tokio-yamux --offline`: 30 library tests passed. The `window_update_deadlock` integration test could not start for the same denied loopback bind; therefore the full crate test command did not pass in this environment. The benchmark's raw TCP and yamux echo paths remain unverified at runtime here.

Re-run the Criterion command and the crate tests in an environment that permits
loopback sockets before treating this as an accepted performance baseline.

## Measured results (rk, outside sandbox, 2026-10-05)

Command: cargo bench -p tokio-yamux --bench bench -- --sample-size 10 --measurement-time 1 --warm-up-time 1
(Ran over direct SSH because the Codex sandbox denies loopback socket bind.)

- loopback_echo_512k/raw_tcp/512_kib: 2.19 / 2.28 / 2.35 ms per 512 KiB transfer
  (approx 219 MiB/s on this box)
- loopback_echo_512k/yamux/512_kib: 14.78 / 17.44 / 20.18 ms per 512 KiB transfer
  (approx 28.7 MiB/s)

Baseline only: yamux echo runs at roughly 13% of raw TCP on local loopback for
512 KiB messages on rk. No protocol or performance change is claimed; these
numbers are the repeatable reference for future comparisons. The earlier yamux
echo read_exact UnexpectedEof panic from the previous round did not reproduce.
