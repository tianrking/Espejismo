# TLS 1.3 early data boundaries

Espejismo uses rustls only for the optional HTTPS egress proxy. The core tunnel
does not use TLS. HTTPS CONNECT is stateful: sending it as TLS 1.3 early data
could replay a proxy action, so the production client must wait for the
authenticated handshake. Rustls' server-side early-data allowance is a byte
limit; zero is the strict boundary that rejects all early application data.
This follows rustls' explicit early-data configuration model and keeps the
project's native, non-impersonating tunnel unchanged (see
`docs/research/REFERENCES.md` and `docs/POSITIONING.md`).

## Change

Made the HTTPS proxy `ClientConfig` explicitly set `enable_early_data = false`
instead of relying on the library default. Extended regression coverage to
assert the production client setting and the test server's zero-byte early-data
limit. The TLS 1.3 ticket test uses an early-data-enabled client against a
server issuing zero-byte tickets, verifies early data is not accepted on the
full or resumed connection, and checks ticket replenishment after resumption.
This tests the configuration boundary; it does not claim to exercise transport
of an early application payload. No performance benefit is expected; this is a
security/correctness boundary change with zero steady-state data-path overhead.

## Validation

`cargo test --offline -p espejismo-server http_chain::tests::https_proxy -- --nocapture`
passed all 8 matching tests, including zero-early-data configuration, TLS 1.3
full/resumed sessions, ticket replenishment, and the production HTTPS proxy
configuration. `cargo fmt --check` remains blocked by pre-existing formatting
differences across unrelated workspace files (including client, core, and
server files); no whole-workspace reformat was applied. Correctness change has
no throughput claim and no expected performance change.
