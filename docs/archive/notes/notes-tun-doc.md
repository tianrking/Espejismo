# TUN documentation

## Findings

- `docs/deployment/TUN.md` already described the platform route managers,
  DNS ownership, cleanup, UDP limitations, and Windows Wintun troubleshooting.
- The support matrix still referred to `v0.1.3`, while the project version is
  `v0.1.5` and the IPv6 limitation remains current.
- The guide had detailed configuration and recovery material, but no compact
  first-run sequence that checks configuration and reachability before starting
  a privileged route/DNS takeover.
- The configuration defaults confirm UDP/443 is blocked and TUN UDP otherwise
  defaults on; the quick-start example explicitly disables UDP for a simple
  TCP-first baseline.

## Change and rationale

Added a quick-start section to `docs/deployment/TUN.md` with a local-only TOML
example, preflight commands, platform privilege reminder, and basic traffic
validation sequence. Replaced the stale release reference with the current IPv4
route support boundary.

Expected benefit: operators can reach a cautious first TUN session without
piecing together the longer platform-specific notes. This documentation-only
change has no runtime or throughput effect; percentage improvement is not
applicable. It preserves the optional TUN ingress and existing proxy-first,
small-operations model.

## Verification

Cross-checked the example fields and defaults against
`crates/espejismo-core/src/config/types.rs` and `defaults.rs`, verified the
existing CLI checks in `docs/deployment/CLI.md` and TUN support notes, and ran
`git diff --check`. No code changed, so Cargo tests and throughput benchmarks
are not applicable; the documentation change has no performance claim.
