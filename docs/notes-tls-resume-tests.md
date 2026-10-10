# TLS session resumption boundary tests

## Findings and approach

The HTTPS proxy client keeps one Rustls `ClientConfig` across connections, so
its session store can resume both TLS 1.3 ticket sessions and TLS 1.2 sessions.
Existing coverage proves TLS 1.3 ticket reuse/replenishment and that the shared
cache is partitioned by server name, but did not exercise the TLS 1.2 path.
Rustls documents TLS 1.2 session ID/ticket resumption as enabled by default in
its `Resumption` configuration; the project reference list has no TLS-ticket
implementation to borrow from the listed proxy projects.

Added an in-memory TLS 1.2-only server/client regression test. It performs two
connections with the same client config and asserts that the first is a full
handshake and the second is resumed. This changes test coverage only: no
production behavior, protocol, dependencies, or positioning changes. Expected
performance change is none; the test guards the existing reuse capability.

## Verification

- `cargo test --offline -p espejismo-server https_proxy_tls12_sessions_resume_with_shared_config -- --nocapture`: passed. Confirms full-then-resumed TLS 1.2 handshake using `tokio::io::duplex`, without loopback sockets.
- `cargo test --offline -p espejismo-server`: passed, 53 passed, 0 failed, 1 ignored (`relays_tcp_through_two_socks5_hops`, requires loopback bind). Existing TLS 1.3 ticket replenishment and server-name cache partition tests also passed.
- No throughput comparison applies because this is a correctness test only.
