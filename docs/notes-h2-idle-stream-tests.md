# HTTP/2 idle and half-closed stream tests

## Findings and approach

`crates/espejismo-core/src/underlay.rs` creates one HTTP/2 request stream per
underlay connection and adapts its DATA body to a Tokio duplex byte stream.
The adapter maps application write shutdown to HTTP/2 `END_STREAM`; the h2
crate owns stream state, reset handling, and connection scheduling. Existing
tests cover reset cleanup, trailers, and flow-control stalls, but did not pin
the half-closed state where one body direction reaches EOF while the response
direction remains active.

The references list points to Xray-core for underlay abstraction ideas and
requires preserving the real HTTP/2 transport semantics. Espejismo's own
positioning likewise treats HTTP/2 as a genuine optional carrier. Because an
underlay uses one stream per connection, an independent concurrent-stream
cap or stream scheduler would add behavior outside this adapter's current
model. The change therefore tests lifecycle behavior at the adapter boundary
without introducing a new limit or timeout.

## Changes and expected effect

- Add an in-memory regression test that shuts down the client write half,
  confirms the server sees EOF, and then sends a complete response in the
  reverse direction.
- Bound both EOF observations with a one-second timeout so a stuck idle pump
  fails deterministically rather than hanging the suite.
- Runtime behavior and throughput are unchanged; expected performance delta
  is 0%. The test verifies half-close handling and eventual cleanup via EOF.

## Validation

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core
  http2_underlay_half_close_keeps_reverse_direction_alive -- --nocapture`:
  passed. This covers client END_STREAM propagation, continued server-to-client
  response data, and EOF in both directions.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core --quiet`:
  passed (342 unit tests, 1 ignored; 1 config example test, 10 HTTP proxy
  tests, and 1 doctest). The ignored case is loopback-dependent and excluded
  under the sandbox rule.

No performance benchmark applies because this is regression test coverage only
and no runtime behavior changed.
