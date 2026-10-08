# HTTP/2 server push boundary test

## Findings and approach

The HTTP/2 underlay in `crates/espejismo-core/src/underlay.rs` carries tunnel
bytes in one POST request/response stream; it has no use for server push. The
adapter delegates HTTP/2 semantics to the `h2` crate instead of implementing
its own frame codec. This follows the existing Xray-core transport-adapter
reference: reuse a real protocol implementation and keep the tunnel's
positioning and protocol unchanged.

The h2 client API exposes an `enable_push(false)` setting. Testing a peer that
disallows push exercises an important interoperability boundary: the server's
push operation must return an error rather than being treated as a usable
stream. The test uses Tokio's in-memory duplex transport, so it requires no
loopback socket.

## Changes and expected effect

- Add a regression test in `underlay.rs` that sends a request from a client
  with push disabled and confirms `push_request` is rejected.
- No runtime behavior or performance change is intended; throughput impact is
  expected to be 0%. The test protects the underlay from relying on an
  unsupported HTTP/2 feature.

## Experiment

- `cargo test -p espejismo-core --offline http2_server_push_rejects_peer_disabled_push -- --nocapture`:
  passed (1 test). Covers request acceptance and server push rejection when
  the peer advertises push disabled.
- Full `cargo test -p espejismo-core --offline`: passed; 245 unit tests and all
  integration/doc tests passed, with 1 existing loopback-bind test ignored.
- This correctness-only change does not call for a throughput benchmark.
