# Keepalive documentation

## Scope and findings

This is a documentation-only clarification. The configuration reference listed
the two `shared.tcp` timers without explaining their behavior, while the
performance guide mentioned Yamux's inherited default. Source review confirmed
three separate mechanisms:

- `shared.tcp.keepalive_secs` enables TCP socket keepalive when nonzero and
  requests an idle interval where the socket API supports setting it. Probe
  cadence and failure policy remain platform and OS dependent.
- `shared.tcp.heartbeat_secs` defaults to 30 seconds and sends an encrypted
  empty padding frame after an idle wait in the normal framing writer. Zero
  disables that timer; stealth framing follows a separate path.
- Bundled `tokio-yamux` defaults to enabled session ping keepalive every 30
  seconds. The runtime adapter inherits it, and there is no TOML interval
  override.

The reference list's Yamux guidance is relevant: keepalive is a liveness
mechanism, separate from flow-control windows. The existing performance guide
already makes this distinction. No protocol, dependency, or runtime behavior
changed, preserving the product's encrypted, non-camouflage tunnel design.

## Change and expected result

Expanded the accepted-config reference with defaults, zero behavior, scope, and
the distinction between OS TCP probes, encrypted framing heartbeats, and Yamux
session pings. The expected improvement is clearer operator tuning and fewer
mistaken assumptions about which timer detects a stalled connection; runtime
performance is expected to change by 0% because this change only edits docs.

## Verification

Reviewed the descriptions against `crates/espejismo-core/src/tcp.rs`,
`crates/espejismo-core/src/transport/mod.rs`,
`crates/tokio-yamux/src/config.rs`, and the client/server warning checks. No
runtime code changed, so there is no applicable executable regression test or
throughput experiment for this documentation-only task.
