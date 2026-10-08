# WebSocket HTTP cookie header boundary tests

## Findings and approach

There is no dedicated cookie parser in the repository. The WebSocket underlay
uses `parse_http_headers` for handshake headers, and stores each field as an
opaque string after trimming surrounding whitespace. Cookie semantics such as
splitting pairs or decoding values do not belong to this transport adapter, so
this change adds regression coverage without changing parsing behavior or
introducing a cookie dependency. This keeps the underlay's real HTTP behavior
separate from the authenticated encrypted tunnel protocol, consistent with
`docs/POSITIONING.md`.

The tests cover cookie values with an empty value, semicolon separators and
embedded `=` characters, an entirely empty field value with surrounding
whitespace, and duplicate `Cookie` fields. The generic header parser preserves
the opaque value and rejects duplicate field names case-insensitively. No
performance improvement is claimed; the expected benefit is preventing
accidental normalization or acceptance changes at these boundaries.

`docs/research/REFERENCES.md` was reviewed. Its listed transport projects do
not provide a relevant cookie-parser requirement here: this code only parses
the WebSocket handshake's HTTP header block, rather than application cookies.

## Verification

- `cargo test --offline -p espejismo-core websocket_http_headers_parse_cookie_field_boundaries`: passed (1 test). It covers the cookie value delimiters, surrounding whitespace on an empty value, and duplicate field rejection.
- `cargo test --offline --workspace`: core passed (231 passed, 1 ignored), its integration tests passed (1 + 8), client passed (53), and server passed (52 passed, 1 ignored). The full workspace command did not pass: `tokio-yamux` had 49 passed and 1 failed (`session::test::test_only_write_on_stream`, assertion at `crates/tokio-yamux/src/session.rs:1654`). This is outside the modified core underlay code; the failure prevents claiming a clean workspace gate.
- No loopback-dependent ignored tests were run. No performance change is claimed.
