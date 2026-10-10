# Early data replay rejection

## Findings and approach

The HTTPS proxy reuses TLS 1.3 session tickets to reduce handshake work, but
CONNECT carries stateful proxy credentials and opens a tunnel. TLS resumption
does not require 0-RTT, so replayable requests must remain unavailable until
the authenticated handshake completes. The production client disables early
data, and the proxy test server issues tickets with a zero early-data allowance.

This follows rustls' separation between session resumption and early-data
authorization: a client opting in cannot make a server accept early data when
the ticket has no allowance. It preserves Espejismo's real HTTPS underlay
behavior and does not add camouflage or another protocol.

## Change and expected effect

The TLS 1.3 ticket test now opts its simulated client into early data while the
server continues issuing resumable tickets with `max_early_data_size = 0`. It
checks that neither full nor resumed handshakes report accepted early data.
This guards the server-side replay boundary while retaining coverage that
resumption works. No throughput change is expected; this is a correctness
guardrail.

## Experiment

- `cargo test --offline -p espejismo-server
  https_proxy_session_tickets_are_reused_and_replenished -- --nocapture` —
  passed. Exercises a client configured to offer early data, the server's zero
  allowance, and both full and resumed TLS 1.3 handshakes; neither handshake
  accepts early data, while session resumption still succeeds.
- `cargo test --offline -p espejismo-server` — 41 passed, 1 ignored, 0 failed.
- `cargo test --offline -p espejismo-core server_rejects_replayed` — 3 passed.
  Covers replay rejection for the same plain first packet, the authenticated
  hello in a fresh envelope, and stealth first packets.
- `cargo fmt --check` was attempted but reports pre-existing formatting drift
  across unrelated files, as well as import ordering in `http_chain.rs` that
  rustfmt would rewrite. No formatter was run across the workspace to avoid
  touching unrelated source.

Conclusion: the TLS endpoint keeps tickets resumable while refusing 0-RTT,
and existing core replay tests continue rejecting captured first packets.
There is no performance claim because this is a protocol correctness change.
