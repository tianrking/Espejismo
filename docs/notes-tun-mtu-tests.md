# TUN MTU test coverage

## Findings and approach

The TUN device and the `netstack-smoltcp` stack both receive the configured
`local.tun.mtu`. The existing client tests covered UDP queue limits but did not
exercise netstack construction at different MTUs. `netstack-smoltcp 0.2.2`
uses smoltcp 0.12 with IPv4/IPv6 enabled, but does not enable smoltcp's
`proto-ipv4-fragmentation` feature. Its MTU configures device capabilities and
TCP socket sizing; this does not establish IP fragment generation or
reassembly.

Following the references' preference for explicit protocol boundaries and
small, executable regressions, this change makes the `u16` to `usize` conversion
explicit at the netstack boundary and adds a build regression for MTUs 576,
1280, 1400, and 1500 bytes. No fragmentation support or tunnel behavior is
claimed or introduced, preserving the existing TCP/yamux TUN design and
project positioning.

Expected benefit: catches a future regression where common configured MTUs
cannot initialize the userspace stack. Runtime and throughput improvement are
not expected; this is a correctness/robustness test task, so percentage
performance gain is not applicable. The test checks stack construction only,
not live packet fragmentation or path-MTU discovery.

## Verification

- `cargo test -p espejismo-client userspace_netstack_builds_with_common_tun_mtu_values`:
  passed for all four MTUs.
- `cargo test -p espejismo-client`: 44 passed, 0 failed. This includes the new
  MTU stack-build test and existing TUN UDP queue/backpressure tests.
- No throughput benchmark was run because this change makes no performance
  claim and does not alter packet processing.

## Open boundary

If actual IP fragmentation/reassembly coverage is required, it needs either a
netstack feature/dependency change with resource-limit review or an end-to-end
TUN integration harness. The present regression intentionally does not imply
that oversized IP packets are fragmented by the userspace stack.
