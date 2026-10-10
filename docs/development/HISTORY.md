# Project history

This page summarizes how Espejismo has evolved, based on its published release
records. It is an overview for readers who want the major decisions and stages;
the [changelog](../../CHANGELOG.md) remains the detailed record of release
changes.

## From encrypted proxy to operational tunnel

The initial `v0.0.1` release established the core: a native Rust workspace, an
encrypted TCP tunnel with yamux streams, local SOCKS5 and HTTP proxy ingress,
and X25519 plus XChaCha20-Poly1305 protection. Replay checks, a client puzzle,
padding, and basic operations support were present from the start. `v0.0.2`
expanded the prototype into a more usable service with multi-user credentials,
quotas, chained SOCKS5 egress, runtime administration, and profile/config
import and export. UDP reliability and congestion-control primitives also
appeared, while the physical tunnel remained TCP based.

## Hardening the native transport and deployment

Releases `v0.0.4` through `v0.0.9` concentrated on safe operation and clearer
transport boundaries. The project added socket tuning, pacing, bounded mux and
connection resources, diagnostics, fuzz targets, a lane pool, and selectable
native mux work while retaining yamux as the production default. Native TUN
ingress and platform route/DNS management extended traffic capture beyond local
proxies. The protocol documentation made the no-impersonation direction
explicit; the standard handshake envelope was masked, and the transport
architecture was made replaceable without changing TCP as the default.

The same period improved installation and recovery: role-aware release
packages, onboarding profiles, route cleanup, and cross-platform TUN guidance
made the two-binary deployment practical. Linux route takeover was moved to a
dedicated policy table, with protection for the remote endpoint and tunnel
warmup before takeover. These changes kept system routing behavior explicit
and recoverable.

## Release line and measured transport options

`v0.1.0` marked the operational release line with simplified release packages,
installers, and an aligned tunnel request format. The following patches
focused on TUN reliability, time-window handshake keys, and configurable
upstream proxy egress. From `v0.1.2`, work increasingly emphasized measuring
and explaining behavior: per-lane counters, repeatable throughput summaries,
and benchmark records informed dispatch changes and adaptive lane scoring.

`v0.1.3` added optional WebSocket and HTTP/2 underlays, deterministic port
hopping, and a measured high-BDP profile, while keeping the same Espejismo
cryptographic stream above those TCP underlays. `v0.1.4` introduced adaptive
lane scoring and richer benchmark observability. `v0.1.5` refined bulk-lane
classification for download-like HTTP requests and made active transfer bytes
visible in the admin data. These releases broadened transport choices and
operational visibility without changing the project's no-camouflage,
small-operations model.

## Continuing direction

The current release target and implemented capabilities are tracked in
[implementation status](STATUS.md); active areas and possible future work are
in the [roadmap](../ROADMAP.md). The release history does not imply that
experimental UDP primitives are an integrated UDP underlay, nor that optional
WebSocket or HTTP/2 transports provide protocol camouflage. See
[project positioning](../POSITIONING.md) for the enduring scope and boundaries.
