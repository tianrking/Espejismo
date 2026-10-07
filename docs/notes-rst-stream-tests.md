# RST stream boundary tests

## Findings and change

The existing reset storm regression verifies prompt session-map cleanup and
that an idle handle observes `ConnectionReset`. A separate stream boundary was
not covered: if DATA was already buffered when an RST arrived, `poll_read`
could return those bytes before reporting the reset. A DATA frame carrying RST
could also append its own payload after transitioning to the reset state.

Yamux treats RST as an immediate stream abort. Following the stream lifecycle
discipline in the yamux reference listed in `docs/research/REFERENCES.md`, the
handle now clears buffered receive data when processing RST and discards the
payload of a DATA+RST frame. This is stream-local and does not affect session
or tunnel protocol identity.

Expected benefit: applications receive a deterministic reset at the boundary
and cannot consume stale or post-reset bytes. This is a correctness fix; no
throughput improvement is expected.

## Verification

On 2026-10-08:

- `cargo test -p tokio-yamux --lib reset_discards_buffered_data_and_data_on_reset_frame --offline` passed. It queues ordinary DATA followed by DATA+RST and verifies the first read returns `ConnectionReset` with no buffered bytes left.
- `cargo test -p tokio-yamux --lib --offline -- --test-threads=1 --skip test_only_write_on_stream` passed: 49 tests, including reset storm, half-close, and the new reset boundary.
- The unfiltered library suite ran 50 tests but failed the unrelated existing `session::test::test_only_write_on_stream`: its final 2 MiB comparison received the initialized zero-filled buffer. Re-running that test alone reproduces the failure. The test does not exercise RST. No throughput benchmark was run because this is a correctness fix with no expected throughput change.
