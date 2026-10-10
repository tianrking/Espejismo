# TUN MTU Boundary Tests

## Scope and rationale

The current TUN path takes its MTU from configuration and passes it to both
`tun-rs` and `netstack-smoltcp`; there is no path-MTU discovery algorithm to
change. Configuration validation enforces a minimum of 576 bytes, and the
configuration field is a `u16`. The existing netstack construction test covers
common values (576, 1280, 1400, and 1500), but the configuration test did not
exercise the exact rejected value adjacent to the minimum or the representable
upper boundary.

The change strengthens regression coverage only: reject 575, accept 576, and
preserve 65535 through config parsing without narrowing. This follows the
reference list's guidance to study sing-box's cross-platform TUN implementation
while keeping Espejismo's existing TUN behavior and minimal configuration
model. The upper-bound assertion covers parsing and the `u16` representation;
it does not claim that every operating system can create a device at that MTU.
No path probing, automatic MTU changes, dependencies, or platform-specific
socket behavior are introduced. Expected performance impact is none; this is a
validation test change, not a performance optimization.

## Verification

- `cargo test -p espejismo-core rejects_invalid_tun_prefix_and_mtu`: passed.
  Covers the adjacent invalid MTU (575), exact minimum accepted MTU (576), and
  maximum `u16` MTU (65535), including preservation of the parsed value.
- `cargo test -p espejismo-client userspace_netstack_builds_with_common_tun_mtu_values`:
  passed. Confirms netstack construction for 576, 1280, 1400, and 1500 bytes.
- Full package runs passed: `cargo test -p espejismo-core` (216 unit tests and
  9 integration/doc-config tests passed; 1 loopback test ignored) and
  `cargo test -p espejismo-client` (50 tests passed).
- No throughput comparison applies: runtime code and packet handling are
  unchanged.
