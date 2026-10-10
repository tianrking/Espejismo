# Ping keepalive boundary tests

## Findings and change

`tokio-yamux` uses a 30-second ping timeout and the configured interval to
schedule keepalive pings. Before this change, an unacknowledged ping timed out
only when its age was strictly greater than 30 seconds; that boundary was not
covered. A zero interval also reached Tokio's interval constructor, which
rejects a zero period and panics.

The implementation now makes those semantics explicit: a ping is valid at
exactly 30 seconds and times out one nanosecond later; configured intervals are
clamped to a 1 ms minimum. This is a narrow robustness guard for malformed
internal config and does not change the normal 30-second default or Espejismo's
protocol positioning. The reference reviewed was the bundled Yamux
implementation (`crates/tokio-yamux/src/session.rs`), which tracks ping send
times and periodically sends keepalive frames.

## Verification

- `cargo test -p tokio-yamux keepalive_timeout_and_interval_boundaries_are_sanitized` — passed. Covers just-before, exact, and just-after timeout ages; zero, minimum, and above-minimum intervals; and construction of a session with a zero interval.
- `cargo test -p tokio-yamux test_keepalive_should_work_on_no_communication_scenario` — passed in 30.20 s, exercising actual keepalive expiry when the peer does not acknowledge pings.
- `cargo test -p tokio-yamux` — all 36 unit tests passed. Its separate `window_update_deadlock` integration test could not run in this sandbox: binding its TCP socket failed with `PermissionDenied (Operation not permitted)` at `crates/tokio-yamux/tests/window_update_deadlock.rs:31`.
- `cargo fmt --all -- --check` reports pre-existing formatting differences in unrelated files; the changed Rust file was formatted directly with `rustfmt --edition 2024`.

No performance claim applies: this change adds no hot-path operation beyond extracting the existing ping age comparison and sanitizing the interval once at session construction.
