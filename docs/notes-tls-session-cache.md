# HTTPS proxy TLS session cache boundaries

The HTTPS proxy client intentionally shares one Rustls `ClientConfig` so TLS
session tickets can be reused across CONNECT connections. The existing ticket
test covers ticket consumption and replenishment for one server name. Add a
boundary regression for Rustls' server-name partitioning: a ticket obtained
for one proxy hostname must not cause a resumed handshake for another hostname
when both use the shared client configuration.

The test uses an in-memory TLS 1.3 server and the existing test certificate
verifier. It performs a full handshake for `localhost`, then a full handshake
for `otherhost`, then confirms `localhost` still resumes. This protects the
shared-cache behavior without changing production code, TLS policy, the
Espejismo protocol, or its non-camouflage positioning. Expected performance
change: none; this is regression coverage only.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-server https_proxy_session_cache_is_partitioned_by_server_name -- --nocapture`: passed. Covers independent cache keys for different SNI values and confirms the original hostname's ticket remains reusable afterward. The test uses Tokio duplex streams and needs no loopback socket.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-server`: passed, 45 passed, 0 failed, 1 ignored (existing loopback-bind test). The new SNI partitioning test and existing TLS ticket reuse/replenishment tests all passed.

The earlier test in `docs/notes-session-ticket-tests.md` covers initial full
handshake, resumption, and ticket replenishment. This change complements it by
checking the server-name boundary; no throughput claim is applicable.
