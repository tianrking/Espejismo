# TCP keepalive boundary tests

## Findings and approach

`shared.tcp.keepalive_secs` is a socket-level TCP option, separate from the
encrypted framing heartbeat and Yamux session pings. Zero disables keepalive
configuration; positive seconds are applied to the connected socket with
`socket2`. Socket setup enables keepalive before connect, then configures the
idle duration once the `TcpStream` exists. The existing config reference
(`docs/research/REFERENCES.md`) points to HashiCorp Yamux for session liveness
practices; this task leaves that session behavior and the project's raw
TCP/yamux positioning unchanged.

Extracted the seconds-to-duration decision into a small pure helper and added
boundary coverage for zero, one second, and `u64::MAX`. This checks disable
semantics and guards against integer overflow while remaining independent of
OS-specific socket limits. The OS may reject a very large duration when it is
actually applied; this change deliberately does not claim otherwise.

## Expected effect

No throughput or wire behavior change. The gain is regression coverage for the
configuration boundary and prevention of accidental overflow in duration
conversion.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core tcp_keepalive_seconds_boundaries_preserve_disable_and_duration --lib --offline` — passed; checks disabled zero, one-second mapping, and maximum `u64` mapping.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline` — passed: 206 unit tests, 1 ignored loopback-bind test, 1 config example integration test, 8 HTTP proxy integration tests, and 1 doctest. The ignored listener test requires loopback bind, unavailable in the sandbox.

No performance claim applies because this is correctness-only test coverage.
