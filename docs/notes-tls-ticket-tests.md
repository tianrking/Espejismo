# TLS session ticket boundary tests

## Findings and plan

The HTTPS egress proxy shares one rustls `ClientConfig`, and in-memory handshake tests already cover TLS 1.2/1.3 resumption, TLS 1.3 ticket replenishment, and server-name cache partitioning. The project does not configure a server-side ticket service for tunnel TLS: TLS is only used for outbound HTTPS proxy connections. Rustls owns ticket formats and rotation; its `ProducesTickets` contract requires authenticated decryption and key rolling/erasure for lifetime enforcement. This matches the small operating model and does not introduce TLS camouflage.

Following rustls' `ProducesTickets` boundary, I added a unit test against the ring provider's recommended ticket producer. It checks that encrypted tickets conceal the plaintext and reject tampered or empty ciphertext. This stays in-memory and does not need loopback sockets.

## Change and expected effect

The production path is unchanged. The test adds regression coverage for ticket confidentiality and integrity with zero throughput or runtime impact. Existing handshake tests continue to exercise ticket issuance, resumption, replenishment, and server-name partitioning.

Rotation and expiry are owned by rustls' `Ticketer`/`TicketRotator`; the recommended ring `Ticketer` uses a fixed 12-hour key lifetime with key rotation, and rustls unit tests cover rotation internals. This crate-level test does not force wall-clock expiry or internal key rotation, and makes no claim that it does. No project-owned ticket lifetime policy exists to test.

## Verification

- `cargo test --offline -p espejismo-server rustls_ticket_ciphertext_is_opaque_and_authenticated` — passed; verifies round-trip decryption, ciphertext differs from plaintext, and tampered/empty tickets fail authentication.
- `cargo test --offline -p espejismo-server https_proxy_session_tickets` — passed; existing TLS 1.3 session ticket reuse, replenishment, early-data rejection, and TLS 1.2 resumption checks remain green.
- `cargo test --offline -p espejismo-server` — passed, 67 passed, 0 failed, 1 ignored. The ignored test requires loopback bind; all in-memory TLS ticket tests ran.
