# HTTP/2 server push boundary tests

## Findings and approach

The HTTP/2 underlay in `crates/espejismo-core/src/underlay.rs` carries tunnel
bytes in one POST request/response stream; it has no use for server push. The
adapter delegates HTTP/2 semantics to the `h2` crate. As in Xray-core's
transport-adapter approach noted in `docs/research/REFERENCES.md`, tests should
exercise the real protocol library while leaving Espejismo's tunnel behavior
and positioning unchanged.

The `h2` client can disable push, and the server API reports that restriction
when asked to create a push promise. HTTP/2 push requests must use safe,
cacheable methods; `h2` rejects a POST promise before creating a stream. On an
enabled peer, a promise can be cancelled before response headers; after
`RST_STREAM(CANCEL)`, starting its response must fail. These tests use Tokio
in-memory duplex streams, so no loopback socket is required. Existing
flow-control coverage transfers 512 KiB in each direction with minimum stream
and connection windows to verify window updates.

## Changes and expected effect

Added a server boundary test that rejects an unsafe POST push request. Together
with the existing disabled-peer and accepted-then-cancelled cases, this covers
push refusal, promise creation, cancellation, and response-after-cancel
rejection. Existing flow-control tests cover window replenishment and recovery
when the stream window is exhausted. No runtime behavior or performance change
is intended; throughput impact is expected to be 0% because only tests changed.

## Experiment

- `$HOME/.cargo/bin/cargo test -p espejismo-core http2_server_push -- --nocapture`:
  passed (3 tests). Covers client-disabled refusal, enabled push cancellation
  and response rejection after reset, and unsafe POST promise rejection.
- `$HOME/.cargo/bin/cargo test -p espejismo-core`: passed (339 unit tests, 1
  ignored, 1 config-example integration test, 10 HTTP proxy integration tests,
  and 1 doctest). HTTP/2 flow-control tests passed, including large transfers,
  window replenishment, and recovery after stream-window exhaustion.
- `cargo fmt --all` was used during test editing; unrelated formatter-only
  changes were discarded. The final diff contains only the focused test change.
- This correctness-only change does not call for a throughput benchmark.
