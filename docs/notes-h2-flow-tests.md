# HTTP/2 flow-control boundary tests

## Findings and approach

`underlay.rs` delegates HTTP/2 window accounting to the `h2` crate, but its
adapter previously called `SendStream::send_data` without reserving send
capacity. That works while the initial credit is available and can stop once a
stream or connection window is exhausted. The receive pump already returns
capacity after copying data to its bounded duplex stream, so the adapter should
wait for that credit and continue sending.

This follows the explicit capacity reservation and update model used by the
Rust `h2` API; Espejismo keeps its existing HTTP/2 underlay and encrypted tunnel
semantics. The change reserves and polls capacity before sending each chunk,
and closes the application read side if the receive pump cannot return credit.
Expected benefit: transfers larger than the initial window continue without
stalling or dropping the remainder. No throughput gain is claimed.

## Validation

Added `http2_underlay_replenishes_exhausted_flow_control_windows`: an in-memory
HTTP/2 client/server pair uses the minimum 65,535-byte stream and connection
windows and transfers 512 KiB in each direction, checking full byte integrity.
This exercises both exhaustion and subsequent `WINDOW_UPDATE` handling without
loopback sockets.

`$HOME/.cargo/bin/cargo test --offline -p espejismo-core http2_underlay_replenishes_exhausted_flow_control_windows -- --nocapture`
passed (1 test). `$HOME/.cargo/bin/cargo test --offline -p espejismo-core`
passed: 282 unit tests, 1 config example test, 10 HTTP proxy tests, and 1
doctest; 1 pre-existing loopback test was ignored as required by the sandbox.
No performance benchmark applies to this correctness fix.
