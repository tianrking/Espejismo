# HTTP proxy header size limit

## Findings and approach

`accept_http_proxy_with_auth` capped accumulated HTTP proxy headers at 32 KiB,
but checked the cap only before each 2 KiB read. A peer could therefore send a
larger header in the last read, and a terminator beyond the nominal cap could
be accepted. The native mux frame decoder already demonstrates the applicable
pattern: validate the declared/accepted size before consuming the body.

Keep the existing 32 KiB policy and read no more than the remaining byte
budget. The cap includes the terminating `\r\n\r\n`. This bounds parser input
without changing proxy behavior for headers within the documented implementation
limit, auth, or tunnel semantics. Expected outcome: strict memory/input bound;
no throughput change claimed or measured. This is robustness work.

Reference review: `docs/research/REFERENCES.md` identifies yamux and the
protocol-focused Rust proxy implementations as useful bounded-parser references.
The existing native mux framing code applies an explicit maximum before payload
allocation; this change uses the same size-before-acceptance principle and does
not introduce a new protocol or alter Espejismo's positioning.

## Changes

- Added a named 32 KiB HTTP proxy header cap and restricted each incremental
  read to the remaining budget.
- Added in-memory duplex regression cases for a header exactly at the cap and
  one byte over it.

## Verification

- `cargo test -p espejismo-core --test http_proxy`: passed, 8 tests. New cases
  prove exact-limit CONNECT headers are accepted and limit-plus-one headers
  are rejected.
- `cargo test -p espejismo-core`: passed, 200 unit tests, 1 config-example
  integration test, 8 HTTP proxy integration tests, and 1 doctest; 0 failed.
  One existing loopback-bind test was ignored as required by the sandbox rule.
- No performance benchmark was run because this change is a correctness and
  input-bounds fix; no performance improvement is claimed.
