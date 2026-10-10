# Frame ID wrap

## Scope and rationale

The native mux assigns 32-bit stream IDs by parity and increments by two. Its
old checked increment rejected the current ID whenever the *next* increment
overflowed, leaving the final odd/even ID unusable. The cursor now uses `u64`
internally and converts only when issuing a wire ID. This preserves the wire
format and exhaustion behavior while allowing `u32::MAX` for clients and
`u32::MAX - 1` for servers. After that allocation, another open is exhausted;
there is no wrap to a reused ID. This bounded correctness change follows the
explicit ID allocation/exhaustion approach used by yamux rather than recycling
IDs, which could collide with live or delayed frames.

The change is local to Espejismo's native mux. It does not alter the product's
transport or protocol positioning. Expected improvement: correctness at the
terminal ID boundary; no throughput change is expected or claimed.

## Verification

`cargo test -p espejismo-core native_stream_id_allocator_uses_last_ids_before_exhaustion`
passed (1 test), verifying both parity sequences include their last valid wire ID
and then report exhaustion. `cargo test -p espejismo-core mux::native::tests`
passed (21 tests), including native mux open/data, drain, flow control, and ID
boundary coverage. The full `cargo test -p espejismo-core` passed: 162 unit,
1 config example, 4 HTTP proxy, and 1 doc test. No performance claim is made.
