# UDP MTU and Payload Boundary Tests

## Findings and scope

The production UDP relay carries one UDP payload in a length-prefixed request on
the authenticated TCP/yamux tunnel; it does not use the experimental UDP
underlay. The payload length is 16 bits, and SOCKS5 fragment reassembly must
respect that same 65,535-byte ceiling. TUN passes its configured MTU to
`tun-rs` and `netstack-smoltcp`; the userspace stack does not enable IP
fragmentation, and the project has no path-MTU discovery algorithm.

The references list points to sing-box for cross-platform TUN implementation
and shadowsocks-rust for bounded UDP packet handling. This change applies the
bounded-length principle without importing another transport or changing
Espejismo's protocol. A shared `MAX_UDP_PAYLOAD_LEN` constant now governs the
tunnel writer and SOCKS5 reassembler. Tests exercise exactly-at-limit success
and over-limit rejection. The TUN MTU build test remains a construction check;
it does not claim to measure network MTU discovery, IP fragmentation, or
platform device creation.

Expected performance impact is neutral: this is a shared-boundary correctness
change, with no added packet processing on the hot path beyond using a constant.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core protocol::request::tests`:
  passed, 6 tests. The new exact-limit round trip accepts 65,535 bytes; the
  existing 65,536-byte rejection confirms the writer emits no partial request.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core ingress::socks5::tests`:
  passed, 45 tests, including exact-limit reassembly, one-byte-over discard,
  ordered fragments, source isolation, malformed-sequence reset, and timeout.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-client
  userspace_netstack_builds_with_common_tun_mtu_values`: passed for configured
  MTUs 576, 1280, 1400, and 1500 bytes. This checks netstack construction only.
- Full package runs passed: `cargo test --offline -p espejismo-core` (329 unit
  tests passed, 1 loopback test ignored, 11 integration tests and 1 doctest
  passed) and `cargo test --offline -p espejismo-client` (56 tests passed).
- No throughput comparison applies: no relay algorithm or packet processing
  changed, and this task makes no performance claim.
- `cargo fmt --all -- --check` reports pre-existing formatting differences in
  unrelated files across the workspace; no repository-wide formatting was
  applied to avoid unrelated churn.
