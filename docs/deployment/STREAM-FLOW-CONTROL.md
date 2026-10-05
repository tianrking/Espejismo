# Stream Flow Control

Espejismo's multiplexers apply byte flow control independently to each logical
stream. A stream may send only within the credit granted by its peer. When
credit is exhausted, its writer waits for a window update; other streams can
continue to make progress over the shared transport. The stream window is not
a TCP congestion window and does not reserve that amount of memory up front.
TCP congestion control and socket buffering still govern the shared connection.

## Yamux (production default)

Each Yamux stream begins with 256 KiB of send and receive credit. Sending data
reduces local send credit; receiving a window update adds the advertised delta.
The receiver checks incoming data against remaining receive credit, queues
accepted data for its application reader, and restores credit as that reader
consumes data. It advertises the increase with a `WINDOW_UPDATE` frame when
enough credit has accumulated (normally at least half the configured maximum),
or when stream control flags need to be sent. This batching avoids a control
frame for every small read while allowing a busy stream to refill its window.

`shared.mux.native_initial_window_bytes` is a historical setting name. In
Yamux mode it sets the maximum stream window, not the initial 256 KiB credit;
the minimum accepted ceiling is 256 KiB and the default is 8 MiB. The window
can grow toward this ceiling through updates. A larger ceiling helps only when
per-stream credit is the bottleneck, for example a long transfer on a high
bandwidth-delay path. It can increase potential buffered data across many
active streams, and is neither a throughput guarantee nor a memory allocation.
Each peer applies its own receive-window ceiling; matching the shared setting
keeps their behavior predictable, but the ceiling itself is not negotiated.

## Native mux

Native mode starts each stream with `shared.mux.native_initial_window_bytes`
of credit. Writes are limited by available peer-granted credit and by the
configured send queue. The receiver returns credit after its application has
consumed a received payload; updates are accumulated while reading a payload
and then queued for the session writer. Per-stream receive channels are also
bounded by `native_stream_buffer_frames`. These frame-count bounds and the
byte window work together: the byte window limits outstanding payload bytes,
while bounded channels and send queues constrain queued frame overhead.

Native mux remains a beta option; Yamux is the recommended production mode.
Its settings and queue behavior should not be assumed to match Yamux's update
threshold or initial credit semantics.

## Tuning

Start with defaults and measure a representative workload. If a single long
stream stalls periodically at its credit limit, increase the window ceiling
only after checking RTT, path capacity, and memory headroom. More streams or
larger windows can increase aggregate in-flight data and buffering pressure;
they do not increase the capacity of the underlying TCP path. Keep shared mux
configuration aligned across peers, and see [Performance Tuning](PERFORMANCE.md)
for profile behavior and benchmark guidance.

Yamux's stream credit follows the window-based multiplexing pattern described
by the upstream [HashiCorp Yamux](https://github.com/hashicorp/yamux) project.
Espejismo keeps this mechanism within its authenticated encrypted tunnel; it
does not use flow control to imitate another protocol.
