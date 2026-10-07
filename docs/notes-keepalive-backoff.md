# Keepalive failure backoff coverage

## Findings and approach

The vendored yamux session reports a missed keepalive as a timed-out session;
when a client later tries to open a stream, the lane's existing error path
records a consecutive failure. Lane reconnect waits use capped exponential
backoff with 80–120% jitter. This follows the bounded retry pacing guidance in
`docs/research/REFERENCES.md` (HashiCorp yamux for session keepalive; quic-go and
hysteria2 for retry handling) while keeping Espejismo's native TCP/yamux
transport and positioning unchanged.

The regression exercises the state transition expected after a keepalive
failure: failure count advances, retry waits increase within the existing
jitter bounds, and a successfully re-established session clears the count.
No retry policy or protocol behavior changes; expected throughput improvement
is 0% because the change is test coverage only. The benefit is preventing
keepalive-triggered reconnects from bypassing the existing retry pacing or
retaining stale failure state after recovery.

## Verification

`$HOME/.cargo/bin/cargo test -p espejismo-client keepalive_timeout_failure_uses_bounded_reconnect_backoff --offline`
passed (1 test). It covers the first 500 ms nominal retry, the second retry's
800–1,200 ms jitter window, and clearing the failure count after a recovered
session. `$HOME/.cargo/bin/cargo test -p espejismo-client --offline` passed all
50 tests (0 failed, 0 ignored), including existing exponential growth, cap,
jitter bound/distribution, and reconnect reset coverage. This state-level test
does not bind loopback sockets. No throughput benchmark applies: runtime retry
timing is unchanged, so measured performance gain is 0% by scope.
