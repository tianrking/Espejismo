# HTTP/2 server push boundary tests

## Findings and approach

The HTTP/2 underlay in `crates/espejismo-core/src/underlay.rs` carries tunnel
bytes in one POST request/response stream; it has no use for server push. The
adapter delegates HTTP/2 semantics to the `h2` crate. As in Xray-core's
transport-adapter approach noted in `docs/research/REFERENCES.md`, tests should
exercise the real protocol library while leaving Espejismo's tunnel behavior
and positioning unchanged.

The `h2` client can disable push, and the server API reports that restriction
when asked to create a push promise. On an enabled peer, a promise can be
cancelled before response headers; after `RST_STREAM(CANCEL)`, starting its
response must fail. These tests use Tokio in-memory duplex streams, so no
loopback socket is required. Existing flow-control coverage transfers 512 KiB
in each direction with minimum stream and connection windows to verify window
updates.

## Changes and expected effect

Added an enabled-peer test that creates a push promise, cancels it, then asserts
a response cannot be sent. Together with the pre-existing disabled-peer test,
this covers rejected and accepted-then-cancelled push boundaries. No runtime
behavior or performance change is intended; throughput impact is expected to
be 0% because only tests changed.

## Experiment

- `$HOME/.cargo/bin/cargo test -p espejismo-core http2_server_push -- --nocapture`:
  passed (2 tests). Covers refusal when client push is disabled and enabled
  promise creation followed by CANCEL reset and rejection of a later response.
- `$HOME/.cargo/bin/cargo test -p espejismo-core`: passed (287 unit tests,
  1 ignored loopback-bind test, 1 config-example integration test, 10 HTTP proxy
  integration tests, and doc tests).
- `cargo fmt --all -- --check` reports pre-existing formatting differences in
  unrelated `espejismo-client` and `espejismo-core/src/admin.rs` files. The
  changed test code was formatted; no unrelated files were modified.
- This correctness-only change does not call for a throughput benchmark.
