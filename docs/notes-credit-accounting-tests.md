# Yamux receive-credit accounting tests

## Research and plan

The workspace reference list points to yamux window management and quic-go's
bounded receive-window accounting. quic-go's independent stream and connection
limits are specific to QUIC and are not being introduced here. This change
keeps Espejismo's existing TCP/yamux transport and validates the local stream
credit invariant: buffered bytes plus advertised receive credit must not exceed
the configured maximum.

The receive-credit calculation previously combined conversion, accumulation,
and subtraction directly in `send_window_update`, leaving boundary paths hard
to exercise without allocating enormous buffers. Extracting that calculation
allows direct tests for conversion failure, sum overflow, buffered credit over
the maximum, and receive credit over available capacity. Window state is now
advanced only after the update event is accepted by the session channel.

Expected effect: no wire or throughput change for valid sessions. Invalid
internal accounting returns the existing invalid-message error without
wrapping, and a closed session channel leaves local credit unchanged.

## Changes

- Extracted checked buffered-byte conversion/summation and receive-credit
  subtraction into `receive_credit_delta`.
- Make receive credit mutation transactional with successful event enqueue.
- Added branch tests for valid arithmetic, conversion and accumulation failure,
  both subtraction failures, flags forcing a below-threshold update, and send
  failure preserving credit.
- Existing threshold and invalid-credit tests continue to cover no-send below
  threshold and invalid stream accounting; existing peer update overflow test
  covers send-side credit overflow.

## Experiment

This is correctness hardening, with no performance claim; valid state retains
the same threshold and emitted delta.

- `$HOME/.cargo/bin/cargo test -p tokio-yamux --lib --offline`: all 35 unit
tests passed. The new tests exercise successful arithmetic, `usize` conversion
failure, accumulated `u32` overflow, both checked subtraction failures, SYN
forcing an update below threshold, and a closed event receiver preserving
credit. Existing tests cover threshold no-op, invalid receive credit, and peer
send-credit overflow.
- `$HOME/.cargo/bin/cargo test -p tokio-yamux --offline`: all 35 unit tests
passed; the integration test phase (`window_update_deadlock`) did not finish in
the sandbox and was interrupted. End-to-end socket transfer remains unverified
here.
- `$HOME/.cargo/bin/cargo fmt --check` reports pre-existing formatting diffs in
unrelated workspace files. `rustfmt --edition 2024 crates/tokio-yamux/src/stream.rs`
completed for the changed Rust file.
