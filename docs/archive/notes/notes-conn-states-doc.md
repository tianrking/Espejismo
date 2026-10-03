# Connection and Stream State Documentation

## Findings

The connection lifecycle is control flow at three scopes, not one shared state
enum: client lane management in `crates/espejismo-client/src/tunnel.rs`,
authenticated physical sessions and stream admission in
`crates/espejismo-server/src/handler.rs`, and per-stream request/egress/relay
handling in the server handler and `relay.rs`. Client runtime snapshots publish
lane health labels such as `connected` and `degraded`; those labels do not
represent negotiated protocol states.

The primary recovery boundary is the physical mux session. A stream-local
request, policy, egress, quota, or idle-timeout failure closes that stream. A
mux or encrypted-frame session failure removes the carrier for every stream on
that session. Client reconnection is lazy on later stream demand, and an
established stream is not replayed or migrated. Native mux `FIN`/`RST` details
are mux-level controls, while Yamux has its own internal stream machinery.

## Change and rationale

Expanded the `Connection and Stream Lifecycle` section in
`docs/ARCHITECTURE.md` with the meaning of published lane labels, a transition
table for client lanes, remote sessions, and logical streams, and a note on the
native mux versus Yamux boundary. This makes failure scope and recovery timing
clear during debugging and avoids suggesting transparent stream resumption.
No protocol, runtime behavior, dependency, or positioning change is intended.

Expected performance improvement: none; this is documentation only. Expected
maintenance benefit: a more precise operational model for distinguishing
stream failures from physical-session failures. This benefit is qualitative
and is not measured as a percentage.

## Verification

Reviewed the state labels and transitions against client lane connection/open
logic in `tunnel.rs`, remote handshake/session admission and request handling in
`handler.rs`, relay behavior in `relay.rs`, and the native mux `FIN`/`RST`
controls in `mux/native.rs`. The architecture text explicitly distinguishes
control-flow labels from wire states. No runtime code changed, so no cargo test
or throughput benchmark was run. Documentation review found no runtime behavior
regression; performance change is not applicable.
