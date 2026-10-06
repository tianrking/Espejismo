# Graceful drain regression coverage

## Scope and approach

The native mux already sends GOAWAY, rejects new streams, and keeps existing
streams until they finish or the configured drain timeout expires. The prior
regression test only sent 12 bytes in one direction, so it did not establish
that queued data in both directions survives drain or that FIN follows all
payload bytes.

Expand that test to keep the stream open across GOAWAY, exchange distinct
256 KiB and 192 KiB payloads concurrently, compare every byte, and verify EOF
after each side sends FIN. This is limited to existing native mux behavior;
process signal handling still exits without coordinating active connections,
as documented in `docs/deployment/SHUTDOWN.md`.

The upstream Yamux implementation is the project's existing mux reference
(`docs/research/REFERENCES.md`); its session shutdown/GOAWAY path provides the
relevant model of refusing new streams while existing streams close. This
change adds no protocol behavior and preserves Espejismo's native encrypted
tunnel and minimal operations model.

## Expected effect

No throughput or runtime behavior change is expected. The regression test now
covers 448 KiB of exact bidirectional delivery and both FIN/EOF transitions,
making loss, truncation, or premature stream closure during native GOAWAY
drain detectable.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core native_mux_goaway_drains_existing_streams` — passed; specifically covers both payload directions, byte integrity, post-FIN EOF, and rejection of new streams after GOAWAY.
- `$HOME/.cargo/bin/cargo test -p espejismo-core` — passed: 159 unit tests, 1 config example integration test, 4 HTTP proxy integration tests, and 1 doc test.
- Performance comparison is not applicable because this changes test coverage only; no performance claim is made.
