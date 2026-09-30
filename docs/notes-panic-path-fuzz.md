# Panic path fuzzing for parse and decode inputs

## Findings and approach

The native mux has two related input paths: `validate_frame_bytes_for_fuzz` handles byte
slices, while `read_frame` reads the same 9-byte header asynchronously and allocates a
payload only after checking the 256 KiB cap. The encrypted frame reader validates decoded
lengths before allocation and uses `read_exact` for truncated data. SOCKS5 UDP parsing checks
address-dependent minimum lengths before indexing. Existing tests cover several representative
errors, but did not systematically exercise empty/truncated headers, maximal declared lengths,
or randomized byte inputs across these public-in-module parser boundaries.

Following the project's framing references (yamux and shadowsocks-rust) and its no-camouflage,
small-operations positioning, this change is test-only: use deterministic bounded random input,
explicit empty/truncated/oversized cases, and `catch_unwind` around synchronous parsers. Exercise
native async `read_frame` on EOF/truncated input and ensure oversized declarations fail before
payload allocation. No protocol or runtime behavior changes are intended.

## Expected impact

No throughput or runtime change is expected. The expected gain is catching panic regressions in
native frame validation/decoding and SOCKS5 UDP address parsing, especially around short buffers
and attacker-controlled lengths. There are no added dependencies.

## Experiment

Pending targeted core test run.

`cargo fmt --all` completed, and `cargo test -p espejismo-core` passed: 117 unit tests and
1 doctest, 0 failures. The randomized SOCKS UDP and native byte-slice cases completed without
panics; native async empty EOF returned `None`, partial headers returned errors, and a
`u32::MAX` payload declaration returned an error before allocation. Performance was not measured
because this is test-only coverage and runtime behavior is unchanged.
