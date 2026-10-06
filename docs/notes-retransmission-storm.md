# UDP Retransmission Storm

## Findings and approach

`UdpReliability::due_retransmissions` previously reset each pending packet's
send timestamp after a timeout, but always compared it against the same
configured interval. Under sustained loss, every unacknowledged datagram could
therefore be sent repeatedly at that fixed rate. The reference guidance in
`docs/research/REFERENCES.md` points to mature loss-recovery state machines in
quic-go and KCP; this change applies the narrow transferable technique of
increasing retry delay while preserving this project's existing UDP framing
and TCP-first tunnel positioning.

## Changes and expected benefit

- Store an independent retry interval for each pending datagram.
- Keep the configured interval for the first timeout, double it after each
  retransmission, and cap it at 16 seconds. Large configured intervals are
  also capped; the packet format and cumulative ACK behavior are unchanged.
- Under prolonged loss, each packet's retransmission frequency falls
  geometrically until the cap, reducing repeated sends by up to 99.4% versus
  the base interval once the 16-second cap is reached (for a 100 ms base).
  This is a theoretical per-packet rate comparison, not a throughput claim.

## Verification

- `cargo test -p espejismo-core protocol::udp::tests`: 5 passed. The new
  deterministic loss test checks exact first through saturated retry deadlines
  and verifies no early resend; existing tests cover ACK removal and delivery.
- `cargo test -p espejismo-core`: 159 unit tests passed, along with its
  integration tests (1 doc-config test and 4 HTTP proxy tests passed).
- `cargo test --workspace --quiet` passed the workspace crates reached before
  `tokio-yamux/tests/window_update_deadlock.rs` failed to create its socket
  pair: the sandbox returned `PermissionDenied (Operation not permitted)` at
  line 31. This test does not exercise the changed UDP scheduler.
- This is a correctness/robustness change in the UDP retransmission scheduler,
  not a measured throughput optimization. No bare-TCP performance comparison
  was run, so no benchmark result or measured performance gain is claimed.
- `cargo fmt --all -- --check` reports pre-existing formatting differences in
  unrelated files. The changed Rust file was formatted directly with rustfmt.
