# TLS handshake timeout

## Findings and plan

The server's inbound Espejismo handshake already runs under the configured `handshake_timeout` in `handler.rs`. The HTTPS egress proxy path was different: `TlsConnector::connect` could wait indefinitely if the proxy accepted TCP but stopped responding during TLS negotiation. The project's reference guide highlights bounded handshake/connection work as an operational resilience technique; this change keeps the existing HTTPS proxy transport and authenticated TLS behavior, without changing the core tunnel protocol or its positioning.

Add a fixed 10-second deadline around only the HTTPS proxy TLS handshake. This bounds resource occupancy for a stalled proxy while leaving TCP connection setup, HTTP CONNECT response handling, and established tunnel traffic semantics untouched. The helper accepts a duration internally so a local regression test can exercise the deadline quickly.

Expected outcome: a stalled HTTPS proxy can hold a connection for at most 10 seconds during TLS negotiation, after which timeout cancellation drops the TLS future and underlying socket.

## Validation

Pending targeted server tests. The regression test uses a loopback TCP peer that reads the TLS ClientHello and stalls; it checks that the handshake returns a timeout and the peer observes EOF, covering both the deadline and cancellation cleanup.

Result: `$HOME/.cargo/bin/cargo test -p espejismo-server --offline` passed (21 tests, 0 failures). The new `https_proxy_tls_handshake_times_out_and_closes_connection` test covers a stalled TLS peer, timeout error classification, and EOF at the peer after cancellation. The existing HTTP and HTTPS CONNECT request tests also passed. Initial loopback-listener setup was denied by the sandbox (`PermissionDenied`), so the regression test uses Tokio's in-memory duplex transport and exercises the same TLS handshake/cancellation path without network access. A repository-wide `cargo fmt --all -- --check` reports pre-existing formatting diffs in unrelated client/core files; the changed Rust file was formatted directly with rustfmt.

Conclusion: bounded HTTPS proxy TLS negotiation and cancellation cleanup are verified; no throughput claim applies to this correctness/robustness change.
