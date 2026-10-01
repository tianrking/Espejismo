# Retry Budget Audit

## Findings and plan

- Client lane reconnects already use exponential backoff with bounded jitter and
  a 16-second ceiling. A single mux stream open is bounded by
  `local.tunnel_pool.max_reconnect_attempts` (default 3); each lane connect,
  including DNS/TCP/handshake, has a 30-second timeout. Permanent accept errors
  terminate the listener loop.
- The server's temporary resource exhaustion path (file descriptors or memory)
  retried every 250 ms forever. That allowed each affected listener to issue up
  to four accept attempts per second during a sustained resource failure.
- Change only this server path: use per-listener exponential delay starting at
  250 ms and capped at 16 seconds, and reset the failure count after a
  successful accept. This maintains automatic recovery for the long-lived
  server while bounding retry frequency. The cap limits this path to at most
  one retry per 16 seconds per listener under persistent exhaustion.

This follows the bounded exponential retry approach used by transport clients
such as Hysteria2 and quic-go, as referenced in
`docs/research/REFERENCES.md`. Only the retry timing technique is applied; no
protocol or positioning changes are involved.

## Expected effect

Under continuous temporary resource exhaustion, retry rate falls from 4/s to
at most 1/16s per listener after backoff growth. A successful accept resets the
next temporary failure to 250 ms, so normal service recovery remains prompt.
Healthy accept throughput is unchanged.

## Implementation and experiment

The accept loop now tracks consecutive temporary resource failures separately
for each listener. The schedule doubles from 250 ms through 16 seconds, remains
capped for further failures, and resets on successful acceptance. Unit coverage
checks the initial delay, growth, cap, and saturation at very large failure
counts.

Validation results:

- `$HOME/.cargo/bin/cargo test -p espejismo-server`: passed, 18 tests (including
  the new retry schedule coverage); 0 failed.
- `rustfmt --edition 2021 crates/espejismo-server/src/main.rs`: applied to the
  edited file. Workspace-wide `cargo fmt --check` reports formatting differences
  in pre-existing, untouched `espejismo-client/src/tun.rs` and
  `espejismo-core/src/transport/mod.rs`; those files were left unchanged.
- No throughput benchmark applies: retry timing runs only when listener accept
  fails from resource exhaustion. Normal accept behavior is unchanged.
