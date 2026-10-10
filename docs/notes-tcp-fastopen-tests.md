# TCP Fast Open boundary tests

## Findings and scope

`crates/espejismo-core/src/tcp.rs` creates ordinary TCP sockets and applies
buffer, keepalive, congestion, and timeout options. The project has no TCP Fast
Open option or early-data path. The existing handshake-latency note
(`docs/archive/notes/notes-handshake-latency.md`) says TFO needs real TCP paths
with TFO enabled and disabled; loopback cannot establish a useful performance
result. The reference projects offer transport and measurement ideas, but none
justify changing Espejismo's raw TCP authenticated-handshake behavior here.

## Change and rationale

Outbound buffer sizes were cast from `usize` to the Tokio socket API's `u32`
without checking. Values above `u32::MAX` could wrap to a different requested
size. The conversion now rejects that boundary with `InvalidInput`. This is a
local TCP socket correctness fix, not TFO enablement, and has no expected
throughput improvement. TFO remains a separate proposal requiring a defined
config/API and non-loopback comparison before implementation.

## Verification

`cargo test -p espejismo-core` passed: 192 passed, 0 failed, 1 ignored
(the pre-existing listener queue test requires loopback bind). The added unit
test covers zero, the largest accepted `u32`, and (on platforms where `usize`
is wider) the first rejected value. No TFO behavior was exercised because none
exists in this codebase.
