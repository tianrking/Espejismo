# Native frame codec property tests

## Findings and approach

The native mux codec lives in `crates/espejismo-core/src/mux/native/frame.rs`. Frames use a
9-byte header (type, big-endian stream ID, big-endian payload length) followed by up to 256 KiB
of payload. An existing byte-slice validator supports the fuzz target and already rejects unknown
types and oversized payloads; there were only two hand-picked parser cases and no property-test
dependency. The project references list includes yamux and shadowsocks-rust for mux/framing
implementation patterns, while the positioning document requires keeping the native encrypted
tunnel model unchanged. This task only strengthens tests and does not change wire behavior.

Use the existing `rand` dependency to generate reproducible-in-structure (bounded seeded-by-test
RNG) samples without adding a dependency: exercise actual asynchronous write/read round trips for
256 random valid frames, and run random byte slices through the validator under `catch_unwind`.
Complete unknown-type headers and an over-limit payload header must return errors. Truncated
headers remain an accepted incomplete-input result (`None`), matching the validator contract.

## Expected impact

No throughput or runtime behavior change is expected. The gain is broader detection of header
ordering, endian, payload preservation, truncation, invalid-type, and panic regressions in the
frame codec, with no added production dependency.

## Experiment

`cargo test -p espejismo-core` passed: 112 tests, 0 failures. The new deterministic sample set
roundtripped 256 random valid frames through the asynchronous codec and checked random inputs up
to 1024 bytes for panic freedom; complete unknown-type and oversized-length headers returned
errors. Runtime/performance behavior is unchanged, so no throughput claim applies.
