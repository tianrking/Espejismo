# HTTP/2 WINDOW_UPDATE boundary tests

## Findings and approach

The HTTP/2 underlay delegates frame parsing and flow-control accounting to the
Rust `h2` crate. The adapter already tests window exhaustion through a paired
in-memory client and server, but did not pin the raw `WINDOW_UPDATE` wire
boundaries. Following the tested transport-adapter model in Xray-core and the
Rust `h2` API's centralized flow-control handling, this change adds raw-frame
regressions instead of introducing a parallel frame parser. The underlay
continues to expose real HTTP/2 behavior and does not change Espejismo's
protocol positioning.

The maximum 31-bit increment is syntactically valid, but adding it to the
default receive window exceeds the protocol's 31-bit window limit. Tests
therefore distinguish that semantic overflow from acceptance of a positive
stream increment; zero increments are checked at both connection and stream
scope. In the tested `h2` version, a zero increment on an existing stream
produces a connection-level `GOAWAY(PROTOCOL_ERROR)` through the server API;
the regression records that observed rejection rather than assuming the
connection remains usable.

Expected benefit: no performance change is intended. The tests guard protocol
error scope and prevent malformed or overflowing credit from being treated as
usable flow-control capacity.

## Implementation and evidence

- Added in-memory raw-wire tests in `crates/espejismo-core/src/underlay.rs` for
  positive stream credit, zero connection credit, rejected zero stream credit,
  and a maximum increment that overflows the connection window.
- Reference reviewed: Xray-core's transport-adapter model, listed in
  `docs/research/REFERENCES.md`; the `h2` crate remains the wire parser and
  flow-control state machine.
- Focused: `$HOME/.cargo/bin/cargo test --offline -p espejismo-core
  http2_window_update -- --nocapture` passed (4 tests). This covers positive
  stream increment, zero connection increment, zero stream increment, and the
  max-increment overflow branch.
- Full: `$HOME/.cargo/bin/cargo test --offline -p espejismo-core` passed:
  311 unit tests, 1 config-example test, 10 HTTP proxy integration tests, and
  1 doctest; 1 pre-existing loopback-bind test was ignored. No failures.
- The zero stream increment probe results in `GOAWAY(PROTOCOL_ERROR)` in the
  current `h2` behavior. This is captured as a decoder rejection regression;
  no Espejismo flow-control implementation was changed.
- No performance benchmark applies to this correctness change.
