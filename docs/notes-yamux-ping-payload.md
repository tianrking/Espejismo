# Yamux ping payload boundaries

The yamux ping value is the 32-bit `length` field in the fixed-size frame header, not a frame body. The session answers a `SYN` ping with an `ACK` carrying the received value. This follows the upstream yamux framing model referenced in `docs/research/REFERENCES.md` (HashiCorp yamux); preserving the opaque ping ID is required for keepalive acknowledgements and does not affect Espejismo's TCP/yamux positioning.

Added a boundary regression test covering IDs `0`, `1`, `u32::MAX - 1`, and `u32::MAX`. It encodes and decodes each SYN, checks that it has no body, then exercises the same ACK construction used by the session and verifies the ID remains unchanged. The frame API comment now documents this field's ping-ID behavior.

Expected impact: no runtime or throughput change; test and API-comment only. The test guards against truncation, signed conversion, or accidental treatment of the ping value as a body.

Validation: `cargo test -p tokio-yamux --offline` compiled the crate and passed all 43 library tests, including `frame::test::ping_id_boundaries_round_trip_and_echo_unchanged`. The package command then failed in the unrelated `window_update_deadlock` integration test because its loopback socket bind was denied by the sandbox (`PermissionDenied`, at `tests/window_update_deadlock.rs:31`); the integration test did not reach protocol behavior. The targeted regression and full library suite passed. No throughput benchmark applies because this change adds no runtime work.
