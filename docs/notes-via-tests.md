# HTTP Via header boundary tests

## Findings and approach

The HTTP proxy currently forwards ordinary request fields as opaque text. That
is the right behavior for `Via`: parsing or rewriting its intermediary chain
would change client supplied metadata without being needed for proxy routing.
The existing parser bounds the full request header at 32 KiB, including the
terminating CRLF pair, and leaves bytes beyond that terminator available as
request body data.

The reference review in `docs/research/REFERENCES.md` points to protocol-focused
proxy implementations and bounded parser practices. Following the existing
bounded-parser approach here, this change keeps the current limit and protocol
semantics, clarifies the opaque-forwarding rule in a code comment, and adds
regression tests at both sides of the size boundary. Expected benefit: protect
the Via chain and body boundary against accidental parsing or normalization;
no performance gain is claimed.

## Changes

- Added a test with a multi-hop `Via` value (including a comma and IPv6
  authority) in a request header exactly 32 KiB long. It checks the value is
  forwarded intact, the header terminator stays in the header buffer, and the
  following body remains readable from the stream.
- Added a test proving a `Via`-bearing header one byte beyond the cap is
  rejected.
- Clarified that forwarding metadata including `Via` remains opaque and is not
  normalized.

## Verification

- `cargo test --offline -p espejismo-core via_header`: passed both new tests.
- `cargo test --offline -p espejismo-core --test http_proxy`: passed all 10
  integration tests covering proxy request parsing, auth, CONNECT, body
  coalescing, and the existing header size boundaries.
- `cargo test --offline -p espejismo-core`: passed 261 unit tests, 1 config
  example integration test, 10 HTTP proxy integration tests, and 1 doctest; 1
  existing test was ignored. No loopback-dependent test was added.
- `cargo fmt --all -- --check` reports formatting differences in pre-existing
  unrelated files; the modified Rust file was formatted directly with
  `rustfmt --edition 2021`.
- This is a correctness and regression-test change; no throughput benchmark was
  run and no performance improvement is claimed.
