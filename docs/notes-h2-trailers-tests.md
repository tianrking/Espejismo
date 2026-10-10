# HTTP/2 trailer boundary tests

## Findings and approach

The HTTP/2 underlay copies only `RecvStream::data()` chunks into the tunnel
adapter. Trailers are HTTP metadata and must not appear as tunnel bytes; when
`data()` returns `None`, the body is finished and the adapter shuts down its
application write half. HTTP/2 frame and HPACK validation remains the
responsibility of the existing `h2` implementation. This follows the
protocol-delegation boundary used by Rust `h2` and the HTTP/2 underlays in
projects such as Xray-core; no protocol or product-positioning change is
needed.

The implementation now documents that trailers are excluded at the body EOF
boundary. Regression coverage sends DATA followed by a legal trailer and
checks that the adapter yields only the DATA bytes; it also confirms a
truncated trailer HEADERS frame is rejected promptly and that pseudo-header
names cannot be constructed as trailer field names through `http::HeaderName`.
The expected gain is correctness coverage for trailer termination and malformed
input handling; no throughput change is claimed.

## Validation

`$HOME/.cargo/bin/cargo test --offline -p espejismo-core http2_underlay_ignores_trailers_and_rejects_invalid_trailer_names -- --nocapture`
passed (1 test). `$HOME/.cargo/bin/cargo test --offline -p espejismo-core http2_rejects_truncated_trailer_frame -- --nocapture`
passed (1 test). Full `$HOME/.cargo/bin/cargo test --offline -p espejismo-core`
passed: 295 unit tests, 1 ignored loopback test, 1 config example test, 10 HTTP
proxy tests, and 1 doctest. No loopback access was needed. No performance
benchmark applies to this correctness-only change.
