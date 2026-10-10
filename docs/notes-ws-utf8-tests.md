# WebSocket Text Frame UTF-8 Boundaries

## Change and rationale

The WebSocket adapter carries tunnel bytes in binary frames. Its parser previously
rejected text frames as unsupported opcodes without checking their payloads.
It now validates text payloads as UTF-8 first, then rejects valid text frames so
they cannot enter the binary tunnel stream. This follows RFC 6455's text-message
encoding rule while preserving Espejismo's existing underlay behavior and
positioning. The protocol documentation records this boundary.

Tests cover isolated invalid bytes, truncated two-, three-, and four-byte
sequences, an overlong encoding, and valid Unicode control code points. They
verify malformed data reports the UTF-8 error and valid UTF-8 still reports the
binary-only policy error.

The references review found no reason to add a dependency or imitate another
protocol's behavior: this is bounded parser validation, consistent with the
existing fail-closed parsing approach described in `docs/research/REFERENCES.md`.

## Validation

- `cargo test --offline -p espejismo-core websocket_text_frames_validate_utf8_before_rejection` — passed (1 focused boundary test).
- `cargo test --offline -p espejismo-core` — passed: 320 unit tests, 1 ignored unit test, 1 config example test, and 10 HTTP proxy integration tests. Doc tests completed without failures.
- These parser tests use Tokio in-memory duplex streams; no loopback bind is required.
- No performance claim applies: this correctness change only adds UTF-8 validation to text frames, which are then rejected and are not part of the normal binary tunnel path.

Conclusion: malformed WebSocket text payloads fail explicitly at the UTF-8 boundary; valid text remains unsupported; binary underlay behavior has no regression in the core test suite.
