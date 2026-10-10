# UDP Checksum Boundary Tests

## Findings and approach

The TUN ingress feeds complete IP packets into `netstack-smoltcp`, which owns
UDP parsing and checksum verification. Its shared UDP verifier treats a zero
checksum as "not supplied" for either address family. That is valid for IPv4,
but IPv6 requires a UDP checksum. Keep smoltcp's checksum calculation and
corruption detection, with a small ingress-side guard that rejects zero only
for IPv6. This does not change the tunnel protocol or TUN's IPv4-only route
takeover support.

The reference list points to `shadowsocks-rust` for async UDP relay practices;
this change uses no relay redesign. The relevant boundary is the standard UDP
checksum rule enforced where packets enter the userspace stack.

## Changes and expected effect

- Added `udp_checksum_valid` to enforce non-zero checksums for IPv6 while
  retaining IPv4's optional checksum behavior.
- Added in-memory tests for IPv4 zero checksum acceptance, IPv6 zero checksum
  rejection, valid checksums in both families, and corrupted non-zero
  checksums in both families.
- Updated the IPv6 deployment notes with the TUN stack checksum behavior.

Expected improvement: correctness coverage only; no throughput claim or
performance change is expected.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-client --offline udp_checksum_rules_cover_ipv4_ipv6_zero_and_corruption`: passed (1 test).
- `$HOME/.cargo/bin/cargo test -p espejismo-client --offline`: passed (57 tests, 0 failed, 0 ignored).
- Tests construct smoltcp UDP packets in memory and do not bind loopback
  sockets. Covered valid and invalid checksum branches for IPv4 and IPv6 and
  the IPv4 zero-checksum exception / IPv6 zero-checksum rejection.
