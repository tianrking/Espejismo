# Shutdown path tests

## Scope and expected result

The vendored Yamux session already handles an inbound remote `GoAway` by
marking the peer as closing and replying with `GoAway` if the local side has
not sent one. The missing regression coverage left this remote shutdown path
unverified. Add a session-level test using the existing in-memory mock socket
to assert the normal response and that the session stream terminates. This is
a test-only code change: no runtime behavior or throughput change is expected;
the expected improvement is detecting regressions in remote shutdown handling.

## Verification

`cargo test -p tokio-yamux test_remote_go_away_is_acknowledged_and_ends_session`
passed (1 targeted regression test). `cargo test -p tokio-yamux --lib` passed
all 28 unit tests, including existing local shutdown timeout and stream-open
shutdown coverage. The full `cargo test -p tokio-yamux` run passed all 28 unit
tests but its TCP integration test `one_way_bulk_transfer_exceeding_window`
could not start because the sandbox returned `PermissionDenied` when binding
the local socket. No throughput change is expected for this test-only change;
there is no performance claim.
