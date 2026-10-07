
# Listener backlog behavior

## Investigation and approach

The shared TCP listener helper creates a `socket2::Socket` and passes `1024` to
`listen`. This is a backlog hint: the operating system may cap it, and the
meaning of a full pending-connection queue varies by platform. This matches the
usual listener model in the referenced TCP-based projects; their application
accept loops drain connections while the kernel owns the pending connection
queue. The backlog does not change Espejismo's protocol or transport identity.

Kept the production backlog at 1024 and extracted a private helper that accepts
a backlog hint for a focused test. The test uses backlog 1, delays accepting,
then verifies queued connections can be accepted and a new connection succeeds
after the queue has been drained. It deliberately does not assert an exact
overflow count, which is kernel-specific. Expected benefit: regression coverage
for listener recovery under accept-queue pressure; no throughput gain is
claimed.

## Validation

- `cargo fmt --all`: passed.
- Sandbox: listener socket creation failed with `Operation not permitted` (OS error 1); the Codex sandbox blocks local sockets, so the gate could not run there.
- Verified outside the sandbox on the rk host: `cargo test -p espejismo-core --lib tcp::` passes, including `listener_recovers_after_accept_queue_is_drained`.
