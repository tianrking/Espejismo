# Connection retry policy documentation

## Findings and plan

The implementation has three distinct outcomes that were easy to conflate in
the architecture overview:

- A client lane connect or handshake failure returns an error to the current
  stream request. A later demand retries the lane after exponential backoff.
- Once a lane is connected, a mux stream-open failure is retried in the current
  request up to `local.tunnel_pool.max_reconnect_attempts` (default 3). A
  connect/handshake failure does not loop through this budget.
- The server accept loop retries recognized temporary resource exhaustion per
  listener. Permanent accept errors end that listener loop.

The client reconnect schedule is zero delay before the first connection, then
500 ms after the first consecutive lane error, doubling toward a 16 s cap. Each
reconnect samples an integer multiplier from 80% through 120%, with the final
delay capped at 16 s. The lane failure count is reset after a successful lane
connection; mux open failures also increase the lane error count. Lane connect,
including DNS/TCP/handshake setup, has a 30 s timeout.
Server temporary-resource retries start at 250 ms, double to 16 s, and reset
after any successful accept.

Document these scopes, defaults, and failure boundaries in
`docs/ARCHITECTURE.md` so operators can distinguish request-level retries from
lazy lane recovery. The bounded exponential schedules follow common resilient
transport practice referenced by Hysteria2 and quic-go in
`docs/research/REFERENCES.md`; only the retry technique is relevant here. This
does not change Espejismo's protocol, positioning, or runtime behavior.

## Expected benefit

No performance change is expected. Explicitly documenting when retries occur,
what is bounded, and what is not replayed should reduce operational ambiguity
and prevent interpreting a failed request as an automatic session resume.

## Implementation and verification

Documentation-only change: expanded the architecture description with client
backoff values, jitter, reset semantics, mux-open attempt scope, and server
accept retry behavior. Checked each statement against
`crates/espejismo-client/src/tunnel.rs`,
`crates/espejismo-server/src/main.rs`, and the config defaults. No code or
configuration changed; no test or throughput benchmark applies.
