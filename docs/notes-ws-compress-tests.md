# WebSocket compression negotiation boundaries

The WebSocket adapter is an in-house byte-stream carrier with no compression
implementation or compression dependency. The references list points to the
WebSocket transport layers in sing-box and Xray-core; those implementations
keep carrier behavior below the authenticated tunnel. This change preserves
that boundary and deliberately does not add compression or change the project's
real-WebSocket transport positioning.

The main risk at this boundary is accidentally accepting extension negotiation
without compressor state handling. A `permessage-deflate` negotiation must
agree on `server_max_window_bits`/`client_max_window_bits` and context takeover
parameters in both directions. Since this adapter advertises no extensions,
it now rejects any extension in the upgrade response, regardless of window or
takeover parameters, and rejects RSV1 compressed frames. The server continues
to ignore request extension offers and returns no extension response header.
This avoids introducing unspecified memory and cross-message state costs.

## Changes and expected effect

- Reject extension-bearing successful Upgrade responses, including
  `permessage-deflate` offers with 8-bit windows or either context takeover
  option. This closes a negotiation mismatch that could otherwise feed
  compressed bytes to the tunnel as plaintext frame payload.
- Add a frame regression test for RSV1 and retain no-extension server response
  behavior in the in-memory handshake coverage.
- No throughput gain is expected: this is a correctness and protocol-boundary
  fix, with negligible work on the normal no-extension handshake path.

## Validation

- `cargo test --offline -p espejismo-core underlay::tests::websocket --lib`:
  passed, 17 matched WebSocket tests. Coverage includes uncompressed upgrade
  behavior, rejection of RSV1, and rejection of extension headers with default,
  8-bit-window, and context-takeover parameters.
- `cargo test --offline -p espejismo-core`: passed, 296 unit tests, 10 HTTP
  proxy integration tests, 1 documented-config integration test, and 1 doctest;
  1 existing unit test was ignored.
- No performance claim applies to this protocol correctness change. The normal
  handshake path gains only an extension-header absence check; compressed
  payloads remain unsupported.
