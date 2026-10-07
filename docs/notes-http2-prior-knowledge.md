# HTTP/2 prior knowledge boundary

## Findings and plan

Espejismo's HTTP/2 underlay uses the standard cleartext HTTP/2 prior-knowledge
connection preface, then a POST stream on the configured path. The server
dispatches between HTTP/2 and the native tunnel by peeking at that preface.
The existing probe used one `peek`, which can return only the initial part of
a TCP byte stream; a valid but segmented preface could therefore be routed to
the wrong protocol handler. HTTP/2 implementations commonly validate the
connection preface before processing streams; this change follows that
protocol boundary without changing the underlay or project positioning.

The server now keeps peeking, within the existing probe timeout, while all
observed bytes remain a possible prefix of the standard preface. It rejects
impossible prefixes immediately and accepts only after the full preface is
visible. The matcher regression test covers complete, truncated, empty,
malformed, unrelated, and valid-preface-with-following-bytes inputs.

Expected gain: correctness under segmented TCP delivery; no throughput change
is expected because this is protocol dispatch, not a data path optimization.

## Validation

`$HOME/.cargo/bin/cargo test -p espejismo-core -p espejismo-server --offline`
passed: core 203 passed / 1 ignored, config examples 1 passed, HTTP proxy
integration tests 8 passed, server 44 passed / 1 ignored, and core doctests 1
passed. The ignored tests are pre-existing loopback-bind cases. In particular,
the HTTP/2 preface boundary test passed for all listed complete/truncated/
empty/malformed/unrelated cases. The underlay roundtrip and crypto-handshake
tests also passed.

`cargo fmt --all -- --check` remains non-clean because numerous untouched files
already differ from the installed rustfmt output. The two changed Rust files
were formatted directly with `rustfmt --edition 2021`. This correctness change
has no throughput claim or expected throughput delta.
