# HTTP/2 PING ACK boundary tests

## Findings and approach

The HTTP/2 underlay delegates PING state and wire validation to Rust's `h2`
crate. Existing in-memory coverage checked one outstanding probe, bounded
waiting when the peer is silent, and rejection of PING payloads whose length is
not eight octets. It did not pin the handling of an ACK with no matching PING
or a second ACK for an already completed probe.

Following the raw-frame tests used for other HTTP/2 connection boundaries,
these cases use Tokio duplex streams and a manually encoded ACK. For a valid
probe, the test reads the actual outgoing PING payload and echoes it, so it
exercises ACK matching rather than assuming the opaque payload value. This
keeps HTTP/2 as a genuine transport adapter and makes no runtime behavior or
positioning changes. The reference list's Xray-core entry supports retaining
the transport adapter boundary; the `h2` crate remains responsible for HTTP/2
framing and connection state.

## Changes and expected effect

- Add an unsolicited-ACK case before a probe and a duplicate-ACK case after a
  matching probe completed; neither may report a pong.
- Keep existing coverage for one in-flight probe, caller-bounded timeout, and
  malformed PING payload lengths.
- Runtime behavior and throughput are unchanged; expected performance delta is
  0%. The change adds protocol regression coverage only.

## Validation

- `cargo test --offline -p espejismo-core http2_ping -- --nocapture`: all 4
  PING tests passed, covering successful ACK, unsolicited ACK, duplicate ACK,
  in-flight limit, timeout, and invalid payload lengths.
- `cargo test --offline -p espejismo-core`: 326 unit tests passed, 1 ignored
  (requires loopback bind); 1 config example test passed; 10 HTTP proxy tests
  passed; 1 doctest passed.

All added tests use in-memory duplex streams and require no loopback socket.
No performance benchmark applies because no runtime or throughput path changed.
