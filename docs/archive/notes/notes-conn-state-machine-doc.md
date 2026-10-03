# Connection State Machine Documentation

## Scope and findings

This is a documentation-only change. The connection lifecycle is implemented
across the local lane pool (`crates/espejismo-client/src/tunnel.rs`), remote
peer/mux handler (`crates/espejismo-server/src/handler.rs`), and per-stream
relay (`crates/espejismo-server/src/relay.rs`). There is no single shared state
enum: the lifecycle is control flow at listener, physical-session, and logical
stream scopes.

The client creates a lane's authenticated mux control on demand. Underlay
connect and handshake errors return failure; lane backoff and retry limits
bound subsequent attempts. A stream-open error clears the control. When its
mux session ends, accounting is decremented, and a later stream open can
establish a new session. An established stream is not replayed after a session
failure. Max-age rotation is checked on a later open, so it does not promise an
immediate replacement in the absence of demand.

The server rejects authentication failures and timeouts before mux setup. A
session-level mux/frame failure ends the physical peer session; each mux stream
then independently proceeds through request parsing and egress/relay or ends
on policy/quota/connect failure, idle timeout, or I/O error. A zero per-session
stream limit drops a newly yielded stream; global stream permit exhaustion and
per-session permit wait timeout return from the handler and close that physical
session. The listener continues accepting peers. AEAD/frame decoding is
fail-fast, so corruption is a physical-session failure rather than a
stream-local recoverable condition.

## Change and rationale

Added a lifecycle subsection to `docs/ARCHITECTURE.md` with the state scopes,
normal path, and exceptional exits. This makes the recovery boundary explicit
and avoids implying transparent stream resumption. The description preserves
the product's native authenticated-encrypted tunnel and small operational
model; it introduces no protocol, behavior, or dependency change.

Expected performance or behavior improvement: none (documentation only). The
expected maintenance benefit is fewer incorrect assumptions when debugging
reconnects and stream failures; this is not quantified.

## Verification

Reviewed the documented paths against the client lane setup/open logic and
server handshake, mux, and stream handlers. No code changed, so the correctness
test gate does not apply to this documentation-only scope. No benchmark was
run because no performance behavior changed. Result: documentation review
complete; no runtime regression is possible from these edits.
