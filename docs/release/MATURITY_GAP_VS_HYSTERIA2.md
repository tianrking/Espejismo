# Maturity Gap vs Hysteria2 (No-Impersonation Track)

This document tracks maturity gaps against Hysteria2 while keeping Espejismo's
core design fixed: no protocol impersonation, low-feature encrypted transport,
and bounded public-side behavior.

## Principle Lock

Do not change:

- No TLS/HTTP/QUIC impersonation strategy.
- No stable cleartext handshake or frame markers.
- Fail-fast cryptographic integrity and bounded resource defenses.

## Current Strengths

- Native Rust split (`core`/`client`/`server`) with documented protocol.
- Multi-user auth model, quotas, bandwidth policy, egress restrictions.
- Admin reload/apply, runtime metrics, profile import/export.
- Optional TUN ingress with per-OS route/DNS takeover and cleanup path.
- Low-feature transport controls (stealth profile, padding, jitter, key update).

## Gap Summary

Compared with Hysteria2-level operational maturity, Espejismo still needs work
in these areas.

1. Cross-platform TUN soak coverage:
Current status: smoke-level checks exist; long-duration and hostile-network
validation is not complete.

2. Broader operational ergonomics:
Current status: doctor/probe exists; still missing richer "one command"
diagnostic bundles and deeper failure classification.

3. Outbound/auth extension runtime wiring:
Current status: trait interfaces are present; external HTTP/command auth is
implemented in core but not fully wired into remote admission policy.

4. Transport backend diversity:
Current status: transport connector abstraction exists; production remains TCP
underlay only.

5. Real-world scale/chaos test matrix:
Current status: unit and manual smoke coverage exist; standardized e2e,
stress, loss/jitter/MTU, and benchmark automation still need to be rebuilt
around the simplified script policy.

## Implementation Backlog

### P0 Release Safety

- Maintain and execute `docs/release/V0.1.3_CHECKLIST.md` per platform.
- Keep doctor/probe checks mandatory in release scripts for local validation.
- Add CI guard for docs-version consistency (`v0.1.3` lines and release tag docs).

### P1 Reliable TUN Operations

- Add scripted TUN crash-recovery integration test for each supported OS runner
  where available.
- Add explicit regression tests for:
  - route self-protection persistence after reconnect,
  - DNS restore correctness after forced termination,
  - TUN plus ordinary SOCKS/HTTP coexistence under load.

### P1 Extension Wiring

- Integrate external `Authenticator` policy stage after handshake identity
  selection and before stream admission.
- Integrate `TrafficObserver` sinks with pluggable outputs (JSONL/file/admin
  stream), preserving bounded queue behavior.

### P2 Transport Evolution (No-Impersonation)

- Keep TCP as default stable path.
- Add optional experimental underlay backends through `TransportConnector`
  without changing handshake/frame semantics.
- Gate new underlays behind explicit profile flags and separate reliability
  acceptance criteria.

### P2 Test Depth

- Add cross-OS packet-loss/jitter/latency simulations for TUN and non-TUN mode.
- Add long soak runs with auto-rotation and key-update observability checks.
- Add per-feature SLO checks (reconnect time, stream open latency percentile,
  route cleanup success rate).

## Release Readiness Rule

A release is "mature enough" when all P0 items pass on target OSes and no P1
item marked as a regression gate fails. P2 items improve competitiveness but are
not hard blockers for patch releases.
