# Long transfer reproduction diagnosis

## Scope and reference

The reported failure is a deterministic local hang in the encrypted transport
plus Yamux 64 MiB integrity test. The test path is not a network benchmark: it
uses two Tokio duplex connections, authenticated framing, and one Yamux stream.
The positioning remains TCP/Yamux with authenticated encrypted framing; there
is no protocol or product direction change.

The Yamux reference describes a 256 KiB initial per-stream window and requires
window update frames as the receiver consumes data ([HashiCorp Yamux protocol
spec](https://github.com/hashicorp/yamux/blob/master/spec.md)). The local
`tokio-yamux` config sets a maximum stream window, not the initial window.

## Reproduction and change

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

## Isolation experiments

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

## Analysis and next step

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
The 64 MiB correctness test currently fails at its diagnostic deadline, so
there is no passing correctness gate, no before/after throughput claim, and no
commit message for this attempt.
