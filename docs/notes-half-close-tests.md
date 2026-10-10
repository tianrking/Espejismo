# Yamux half-close sequence tests

## Findings and approach

The vendored `tokio-yamux` stream state machine already models `LocalClosing`
and `RemoteClosing`, and the frame definition describes `FIN` as a half-close.
Existing unit tests covered the two directions separately, but did not exercise
one complete request/response sequence across both FINs. The reference list
identifies Hashicorp Yamux as the relevant implementation to follow; this
change keeps the existing Yamux framing and state machine and adds regression
coverage for its directional FIN semantics. This preserves Espejismo's
transport and product positioning.

## Changes and expected benefit

- Added a stream-level regression sequence: client request data, local FIN,
  reverse response data, peer FIN, then read EOF.
- Documented that a Yamux FIN closes only its sender's write direction and
  that the stream remains readable until the opposite FIN.
- Expected benefit: catch regressions that prematurely discard reverse data or
  fail to report EOF after both halves close. This is a correctness-only
  change; no throughput improvement is expected.

## Verification

- `$HOME/.cargo/bin/cargo test -p tokio-yamux test_half_close_preserves_reverse_direction_until_peer_fin --offline` passed. It verified request DATA precedes the local FIN, reverse response DATA remains readable after that FIN, and the second FIN produces EOF and `Closed` state.
- `$HOME/.cargo/bin/cargo test -p tokio-yamux --lib --offline` passed all 37 unit tests, including the existing isolated half-close tests and the new full sequence regression.
- `$HOME/.cargo/bin/cargo fmt --check` reported pre-existing formatting differences in `crates/espejismo-client/src/tun.rs` and `tunnel.rs`; these unrelated files were left untouched. `rustfmt --edition 2021 crates/tokio-yamux/src/stream.rs` completed successfully.
- This correctness change has no expected throughput impact; no benchmark was run.
