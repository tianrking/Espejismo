# Adaptive throughput notes

## Current profiles

The default config uses balanced obfuscation and chunks, 64 bytes of maximum
padding at a 35% chance, a 1 MiB tunnel buffer, a 64 KiB pacing burst and 1 KiB
minimum write, four maximum tunnel connections with one interactive and two bulk
lanes, and a 1 MiB HTTP bulk threshold. TCP_NODELAY defaults on, while the TCP
send and receive buffers are left to the OS. The native mux initial window is
controlled by its default (currently 1 MiB).

`auto-throughput` switches padding and jitter off, uses bulk framing with fixed
chunks from 64 KiB to the 262,127-byte normal payload limit, a 1 MiB pacing
burst and 64 KiB minimum write, a 16 MiB tunnel buffer, at least 4 MiB TCP send
and receive buffers, a native mux initial window of at least 16 MiB, at least
512 streams (up from 256), a 256 KiB HTTP bulk threshold, and six bulk lanes
(one interactive, up to eight connections versus default four). Thus
chunk/framing choices are the same in `fast`
and `auto-throughput`; the latter also raises queues, socket buffers, streams,
and bulk concurrency. These are profile overrides, so pre-existing larger user
values are preserved where the code uses `max`.

## BDP and likely bottlenecks

The in-flight data needed to fill a path is approximately bandwidth times RTT.
At 250 ms RTT, a 100 Mbit/s path has a BDP of 3.125 MB; a 1 Gbit/s path needs
31.25 MB. A 1 MiB queue alone covers only about 34 Mbit/s at that RTT, while a
16 MiB window covers roughly 537 Mbit/s. TCP congestion/window limits, the
mux flow-control window and per-stream buffering can each cap throughput; more
lanes can hide per-stream limits but do not replace adequate aggregate socket
and mux capacity. Small writes, pacing burst size, and frame padding can also
reduce efficiency, but they are less likely to explain a two-order-of-magnitude
gap than a window/queue that is below path BDP.

RTT by itself does not determine BDP: bandwidth must also be estimated or
configured. The prototype therefore assumes a 500 Mbit/s target, computes a
bounded BDP from a caller-supplied RTT sample, and uses it as a minimum for the
native mux window and twice the BDP as a minimum for the tunnel buffer. A later
runtime change should collect multiple handshake or transport RTT samples and
apply this helper before starting bulk streams. Handshake duration is only a
rough RTT proxy because it also includes server and crypto processing; it
should not be treated as a precise network measurement.

## Prototype boundary

`apply_adaptive_throughput` is a deterministic config helper, not automatic
runtime sampling. It starts from the existing `auto-throughput` settings only
at RTTs of 100 ms or more, then raises the native mux window and tunnel buffer
to at least the estimated BDP, bounded at 16 MiB and 32 MiB respectively. The
prototype assumes a 500 Mbit/s target (for example, 250 ms estimates about
15.6 MB of BDP); actual bandwidth estimation is a follow-up. This
keeps short-RTT configurations untouched and leaves deployment-specific
bandwidth estimation and sample filtering for a follow-up.
