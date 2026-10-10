# Connection pool limit boundary tests

## Findings and approach

`local.tunnel_pool` validation already rejected a zero maximum, a minimum above
the maximum, and configured lanes above the maximum. The configured lane counts
were added with ordinary `usize` arithmetic, however, which could wrap for
programmatically constructed configs and undermine the upper-bound check.

The pool layout in the client truncates lane kinds to `max_connections` and
then fills only up to the configured minimum. That behavior matches the hard
pool cap, so this change keeps its policy intact. As in the referenced
yamux/sing-box style of bounded resource configuration, limits remain explicit
and validated at configuration load; no transport or project positioning
changes are involved.

Changed validation to use `checked_add` before comparing the configured lane
count with the maximum. Added config tests for the exact valid boundary
(`min=max=1`, one lane), zero maximum, minimum above maximum, lane sum above
maximum, and zero configured lanes.

Expected benefit: invalid lane arithmetic cannot silently wrap and be accepted;
valid pool behavior is unchanged. This is a correctness/robustness change, so no
throughput gain is claimed.

## Verification

- `cargo test -p espejismo-core config::tests::validates_tunnel_pool_limits_at_boundaries`: passed (1 test); covers exact valid limit and each invalid limit branch.
- Initial `cargo test --workspace` reached the existing `tokio-yamux` loopback integration test, which failed at `TcpListener::bind` with `Operation not permitted` in this sandbox. Marked that loopback-only test ignored with the documented sandbox-external invocation.
- Reran `cargo test --workspace`: passed. Core: 208 passed, 1 ignored; client: 50 passed; server: 45 passed, 1 ignored; yamux unit tests: 45 passed; integration and doc tests passed, with the loopback-only integration test ignored.
- No throughput benchmark was run because this is a configuration correctness change and no performance improvement is claimed.
