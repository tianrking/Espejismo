# TUN checksum and offload checks

## Findings and scope

The TUN bridge in `crates/espejismo-client/src/tun.rs` transfers one IP packet
per async `recv`/`send` call into `netstack-smoltcp`. The bridge currently does
not request TUN offload. `tun-rs` 2.8 documents Linux `.offload(true)` as enabling
GSO/GRO with a virtio-net header; it recommends `recv_multiple`/`send_multiple`,
and its example prepends the virtio header on writes. The current bridge neither
parses those headers nor splits coalesced packets, so enabling the flag alone
would break packet framing. smoltcp emits fully checksummed packets, so its
ordinary packet path does not need checksum completion from the device.

This follows the TUN abstraction and cross-platform approach used by sing-box
(see `docs/research/REFERENCES.md`): use the OS/device backend deliberately and
keep backend-specific fast paths behind their required packet handling. No
protocol, underlay, or product-position change is involved.

## Change and expected effect

Added a comment at the TUN stack boundary documenting why Linux offload remains
disabled, plus a regression test that asks smoltcp to emit an IPv4 header,
checks its checksum, mutates the header, and confirms verification fails. This
guards the checksum invariant relevant to the current packet path. It does not
claim that the test exercises the kernel TUN offload implementation or TCP/UDP
transport checksums.

Expected throughput change: none; this is a correctness guard and leaves runtime
behavior unchanged. Enabling GSO/GRO would require a separately designed batch
and virtio-header-aware bridge, plus Linux end-to-end measurements.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-client tun::tests --offline`: 7
  passed, 0 failed. Covers emitted IPv4 header checksum validity, corruption
  detection, existing TUN MTU construction, and bounded UDP queue behavior.
- `$HOME/.cargo/bin/cargo test -p espejismo-client --offline`: 46 passed, 0
  failed.
- `rustfmt --edition 2021 crates/espejismo-client/src/tun.rs`: completed.
- `$HOME/.cargo/bin/cargo fmt --check`: reports formatting differences in
  multiple pre-existing workspace files (including unrelated core, server, and
  integration-test files); the changed `tun.rs` was formatted directly.

No performance benchmark was run because runtime packet handling is unchanged;
there is no measured throughput improvement to report.
