# HTTPS proxy TLS session reuse

## Findings and approach

The remote's HTTPS upstream proxy path in `crates/espejismo-server/src/http_chain.rs`
created a fresh Rustls `ClientConfig` for every CONNECT tunnel. Rustls stores
resumption state on the client config, so separate connections could not reuse
TLS tickets even when they contacted the same HTTPS proxy. The trusted root set
and hostname verification were already correct and remain unchanged.

Following Rustls' client configuration model, HTTPS proxy handshakes now share
one process-wide `ClientConfig`. Rustls indexes resumption state by server name,
so proxy endpoints retain certificate/name validation while same-proxy
connections can use tickets offered by that server. TLS 1.3 ticket availability
and server policy still determine whether a resumed handshake actually occurs.
No camouflage, protocol change, or new dependency is introduced.

Expected benefit: when the proxy supports session tickets and the client has a
valid ticket, later proxy TLS connections can avoid a full certificate/key
exchange round trip. This reduces setup latency by roughly one network RTT in
the usual TLS 1.3 resumption path; first connections and proxies that decline
resumption see no change. A wire-level latency claim is not made because this
local test environment does not provide a controlled HTTPS proxy endpoint.

## Verification

- `cargo test --offline -p espejismo-server https_proxy`: passed. The new test
  asserts that independent HTTPS proxy handshakes receive the identical shared
  Rustls configuration/session cache. Existing HTTPS handshake timeout and
  CONNECT request tests also passed.
- `cargo test --offline -p espejismo-server`: passed, 26 tests, 0 failures.
- This verifies cache sharing at the project handshake boundary, not successful
  TLS ticket issuance/resumption by a particular proxy. The latter is server
  behavior and requires an integration endpoint that emits tickets.
