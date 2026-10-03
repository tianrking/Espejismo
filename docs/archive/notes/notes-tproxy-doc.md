# Linux TPROXY documentation

## Findings and scope

This is a documentation-only clarification. The repository implements
cross-platform native TUN ingress for system-level capture and route takeover;
it contains no Linux netfilter `TPROXY` listener or `iptables`/`nftables`
interception integration. TUN provides global IPv4 forwarding through the
existing authenticated encrypted tunnel, with TCP and application-level UDP
relay as described in `docs/deployment/TUN.md`. It does not offer the same
kernel socket interception semantics for selectively redirected inbound
connections or original-destination handling.

The docs now identify native TUN as the supported system-level capture path,
link it from the configuration guide, and state this feature boundary directly.
This preserves the existing small-operations model and does not introduce a
new proxy mode or change the positioning in `docs/POSITIONING.md`.

## Expected benefit

Reduce confusion between Linux TPROXY and TUN, and help operators find the
supported configuration. No runtime, throughput, or latency change is
expected (0%); no performance benchmark applies to this documentation change.

## Verification

Reviewed the existing TUN guide, configuration guide, positioning document,
and implementation references in `crates/espejismo-core/src/config` and the
TUN ingress code. Documentation-only scope means no runtime code changed. The
configuration documentation doctest passed: `cargo test --doc -p espejismo-core`
(1 passed, 0 failed). No throughput experiment is applicable because runtime
performance was not changed.
