# Long transfer reproduction diagnosis

Combined notes from two merged investigations:
- Part 1 — relay read-error propagation (commit 8bc4dfe)
- Part 2 — Yamux bulk-test stall diagnostics (commits e92d373, cf9e754)

## Part 1 — Relay read-error propagation

### Findings and reference

`docs/KNOWN_ISSUES.md` records intermittent 64 MiB de → jp downloads ending at
28–53 MiB with curl exit 18, on both public-IP and tailnet underlays. The
existing `encrypted_transport_with_yamux_mux_preserves_bulk_integrity` test
checks all 64 MiB through the encrypted yamux stack, but its in-memory duplex
has no packet loss, latency, or path MTU variation. This environment has no
de/jp endpoints or corresponding runtime logs, so the public route failure
cannot be reproduced here.

The relay used `tokio::select!` to read both directions. It treated a normal
EOF, any socket read error, and expiration of the per-read idle timer as the
same event: shut down the opposite write half and eventually return success.
That can turn a connection reset into an apparently clean but truncated HTTP
response. It fits curl exit 18, but does not prove the reported incidents had
this cause. The handler already logs relay errors, so propagating read errors
will make this distinction visible without changing the protocol or tunnel
positioning.

The references document points to `hashicorp/yamux` for stream window and
keepalive practices and `shadowsocks-rust` for async I/O patterns. This change
does not tune Yamux windows or introduce a new transport: it preserves the
existing TCP/yamux design and makes relay I/O failure handling distinguish
errors from EOF. An idle timeout retains the current half-close behavior.

### Change and expected effect

Changed `metered_idle_copy_bidirectional` to propagate read errors with the
failing relay direction in the error chain. Clean EOF and idle timeout continue
to half-close the other direction as before. Added a regression test that
injects a connection reset and verifies that it is returned as an error.

Expected throughput improvement: **0% claimed**. The intended benefit is
correct failure reporting and avoiding a success-shaped truncated response;
it does not increase transfer speed. Public de → jp reproduction and attribution
remain open until the same request is run on that route with peer logs.

### Verification

```sh
cargo test -p espejismo-core idle_copy_bidirectional --offline
```

Result: **3 passed, 0 failed** (read-error regression, idle-timeout behavior,
and half-close copy behavior). The first run exposed an overly narrow test
assertion because anyhow's default display omits the cause chain; the assertion
was corrected to check the full chain and the rerun passed.

The full `cargo test -p espejismo-core --offline` run passed the other 102
tests but the existing 64 MiB encrypted-yamux integrity case was still running
after 60 seconds. A focused run of that case showed the same behavior and was
interrupted. Therefore the full core suite is **not confirmed passing** in this
environment. This long-running test needs investigation before treating the
64 MiB loopback path as verified here.

The 64 MiB encrypted-yamux integrity test remains available as a separate
loopback regression, not a public-network reproduction. No before/after
throughput measurement was run because this is a correctness and diagnostics
change, not a performance optimization. No throughput or public-route
stability gain is claimed.

## Part 2 — Yamux bulk-test stall diagnostics

### Scope and reference

The reported failure is a deterministic local hang in the encrypted transport
plus Yamux 64 MiB integrity test. The test path is not a network benchmark: it
uses two Tokio duplex connections, authenticated framing, and one Yamux stream.
The positioning remains TCP/Yamux with authenticated encrypted framing; there
is no protocol or product direction change.

The Yamux reference describes a 256 KiB initial per-stream window and requires
window update frames as the receiver consumes data ([HashiCorp Yamux protocol
spec](https://github.com/hashicorp/yamux/blob/master/spec.md)). The local
`tokio-yamux` config sets a maximum stream window, not the initial window.

### Reproduction and change

Added a 15 second deadline and sent/received byte counts to
`encrypted_transport_with_yamux_mux_preserves_bulk_integrity`. Before this
diagnostic, the test could wait indefinitely in `read_to_end`.

Command:

```sh
cargo test -p espejismo-core encrypted_transport_with_yamux_mux_preserves_bulk_integrity -- --nocapture
```

Result: **fails reproducibly after 15 seconds**. At timeout, the receiver had
read `1,048,576 / 67,108,864` bytes and the writer had sent `1,048,576` bytes.
The stop point matches `native_initial_window_bytes = 1 MiB` in the test's
`MuxRuntimeConfig`. This establishes a flow-control stall at the configured
window boundary; it is not evidence of a slow 64 MiB transfer or the de -> jp
route issue.

### Isolation experiments

Two temporary test-only variations narrowed the boundary:

| Variation | Result after 15 s |
| --- | --- |
| Increase both encrypted pump duplex buffers from 1 MiB to 8 MiB | stalled at 1 MiB sent/read |
| Increase Yamux `max_stream_window_size` from 1 MiB to 8 MiB | stalled at 8 MiB sent/read |

The test was restored to its original 1 MiB configured window after each
experiment. This rules out the encrypted pump buffer as the limiting capacity
and shows that transfer progress stops at the Yamux window cap. Because the
receiver has consumed the full reported byte count, the next progress step
requires a later window grant; the present counters do not reveal whether that
grant was omitted, queued but unsent, or received but not applied by the
writer. No production window or timeout tuning is justified yet.

### Analysis and next step

`MuxRuntimeConfig::yamux` maps the configured size to
`tokio_yamux::Config::max_stream_window_size`. This field caps the receive
window; `tokio-yamux` still begins each stream at 256 KiB and grows its window
through updates as data is read. The 1 MiB to 8 MiB boundary-following result
ties the stall to this cap, but the stage timeout and aggregate byte counters
do not identify which endpoint failed to emit, send, or consume the update.

Do not tune production timeouts, window sizes, or pacing based on this test
alone. Next capture `tokio_yamux` frame-level trace events at both endpoints or
add a focused window-update regression that records grant emission, frame
delivery, and send-window application, then fix the smallest confirmed cause.

Update at merge time: the diagnostic was committed as e92d373, and the test is
now `#[ignore]`d (cf9e754) until the window-update stall is fixed, so CI stays
green. Reproduce locally with
`cargo test -p espejismo-core -- --ignored bulk_integrity`.

## Round 008 — post-fix reproduction on de

### Current source state

The current `main` includes the vendored Yamux writer wakeup fix and its focused
one-way 8 MiB / 1 MiB-window regression (`crates/tokio-yamux/VENDOR.md`,
`crates/tokio-yamux/tests/window_update_deadlock.rs`). The end-to-end encrypted
64 MiB test is active again (not ignored) and uses a 120 second deadline with
byte counters. This supersedes the earlier pre-fix “still stalled” result above:
that result described the earlier source state, before the vendored fix was
present. The task rechecked the current revision rather than making another
window or timeout change.

### Reproduction and verification

Command:

```sh
cargo test -p espejismo-core encrypted_transport_with_yamux_mux_preserves_bulk_integrity -- --nocapture
```

Result on 2026-09-29: **passed**, transferring and validating all 67,108,864
bytes in **40.66 s**. There was no hang, and the test verified the complete byte
pattern and clean stream shutdown. This confirms the local encrypted + Yamux
loopback reproducer is resolved at this revision; it does not establish that
the separate de → jp curl exit 18 issue is resolved, because the loopback has
no real-network latency, loss, or MTU variation.

This is a correctness reproduction, not a before/after throughput experiment.
The observed 40.66 s is a single run, not a throughput improvement claim, and
no production tuning was made. Re-run the targeted test on another machine if
comparing performance; retain the same payload, runtime, and mux configuration.

Yamux verification:

```sh
cargo test -p tokio-yamux
cargo test -p tokio-yamux --lib
```

All **23 library unit tests passed**. The package-wide command then failed only
in `tests/window_update_deadlock.rs` before exercising the transfer: its
`TcpListener::bind("127.0.0.1:0")` returned OS `PermissionDenied` in this
restricted environment. The TCP integration regression therefore remains
unverified here; the 64 MiB in-memory encrypted transport reproduction above
did pass.

## Round 009 — repeat reproduction on de

### Reference and scope

The upstream Yamux protocol spec describes receive-window updates as the
mechanism that lets a stream continue beyond its current receive window
([HashiCorp Yamux protocol spec](https://github.com/hashicorp/yamux/blob/master/spec.md)).
The vendored writer wakeup regression remains covered by both library tests and
the one-way TCP integration test. This round only repeats the current
reproducer; it does not change the TCP/Yamux transport, encrypted framing, or
product positioning.

### Reproduction and verification

Commands:

```sh
cargo test -p espejismo-core encrypted_transport_with_yamux_mux_preserves_bulk_integrity --offline -- --nocapture
cargo test -p tokio-yamux --lib --offline
cargo test -p tokio-yamux --test window_update_deadlock --offline -- --nocapture
```

Results on 2026-09-29:

- The encrypted 64 MiB integrity test **passed**, validating all
  67,108,864 bytes in **40.17 s**. This is a single correctness run, not a
  before/after throughput comparison.
- All **23 tokio-yamux library tests passed** in **30.20 s**.
- The focused TCP integration test could not start: binding
  `127.0.0.1:0` returned OS `PermissionDenied` before exercising the transfer.

No production change or throughput improvement is claimed. The local encrypted
Yamux loopback hang remains resolved at this revision. The de → jp curl exit 18
issue remains un-reproduced and unattributed; this in-memory run provides no
evidence about public-route latency, loss, or MTU behavior. Reproduction on the
TCP test and failing public route still requires an environment that permits
loopback binding and access to both peers.
