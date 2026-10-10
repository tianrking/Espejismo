# HTTPS proxy peer certificate verification

## Findings and plan

`espejismo-server/src/http_chain.rs` creates one shared rustls client
configuration using the bundled Mozilla WebPKI roots and connects with the
proxy hostname as the TLS server name. This preserves normal chain and hostname
validation. The reference guide points to Rust-native implementations such as
shadowsocks-rust and sing-box for transport integration; for this narrow
correctness task, the relevant approach is to keep the TLS library's standard
trust-store validation and exercise rejection at the handshake boundary. No
certificate bypass, custom trust configuration, or tunnel TLS is introduced.

Added a regression test with an intentionally self-signed localhost TLS server
and the production HTTPS proxy client configuration. It verifies that the
client rejects the certificate during the TLS handshake and that the server
handshake terminates. The fixture certificate and key are test-only DER files.
The in-memory duplex transport avoids dependence on loopback socket permissions.

Expected outcome: protect the existing untrusted-peer rejection behavior from
regression, with no runtime or throughput change.

## Validation

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-server https_proxy_rejects_untrusted_server_certificate`: passed; the self-signed peer was rejected.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-server`: passed; 29 passed, 0 failed, 1 ignored. The ignored `relays_tcp_through_two_socks5_hops` test requires loopback bind, as declared in the test itself.

Conclusion: untrusted HTTPS proxy certificates are rejected in the TLS
handshake; the full server package suite passes. This correctness change makes
no performance claim.
