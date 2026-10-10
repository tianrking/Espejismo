# GOAWAY boundary tests

## Scope and rationale

The Yamux session already makes repeated local shutdown requests idempotent and
acknowledges a peer's normal GOAWAY. This round adds boundary coverage for the
GOAWAY reason field and the error shutdown path: defined reason values remain
stable, out-of-range `u32` values map to `ProtocolError`, and a received
`ProtocolError` GOAWAY is acknowledged while the session terminates. This follows
the existing Yamux session design and leaves the tunnel's protocol and product
positioning unchanged.

The change is test-only. It has no expected runtime or throughput effect; the
benefit is catching regressions in reason-code interpretation and graceful
shutdown handling.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p tokio-yamux go_away_codes_keep_defined_values_and_map_unknown_values_to_protocol_error` — passed; covers reason values 0, 1, 2, 3, and `u32::MAX`.
- `$HOME/.cargo/bin/cargo test --offline -p tokio-yamux test_protocol_error_go_away_is_acknowledged_and_ends_session` — passed; covers acknowledgement and session termination for remote `ProtocolError`.
- `$HOME/.cargo/bin/cargo test --offline -p tokio-yamux --lib` — passed, 49 tests.
- `$HOME/.cargo/bin/cargo test --offline -p tokio-yamux` — passed, 49 unit tests and doc tests; `one_way_bulk_transfer_exceeding_window` is ignored because it requires loopback bind.

No performance claim is made because this is test-only coverage.
