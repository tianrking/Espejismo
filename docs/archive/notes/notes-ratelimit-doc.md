# Rate-limit configuration documentation

## Findings and approach

- `shared.pacing` is configured in shared settings used by both binaries. The
  frame sender paces outbound encrypted frame writes when enabled and the rate
  is nonzero; its `max_bytes_per_sec` default is `0` (uncapped). The default
  burst and minimum write charge are 65,536 and 1,024 bytes.
- A one-sided pacing configuration therefore affects that peer's outbound
  traffic. Matching settings on both peers cover both tunnel directions.
- `remote.users[].bandwidth.bytes_per_sec` is the server-side per-user
  aggregate relay limit. Connection and stream limits govern concurrency,
  rather than bytes transferred.

Expanded `docs/deployment/CONFIG.md` to explain these scopes, byte units,
defaults, burst behavior, and how shared pacing and per-user limits combine.
Linked the dedicated user-limit guide and the connection/stream admission
section. This is documentation-only and preserves the no-camouflage, small
operations positioning in `docs/POSITIONING.md`.

## Expected effect

Operators can select a total outbound cap or per-user relay cap and understand
which direction and traffic scope each setting affects. No runtime or
throughput change is expected; numeric performance improvement is not
applicable to documentation-only work.

## Verification

- Cross-checked defaults and validation against
  `crates/espejismo-core/src/config/types.rs`,
  `crates/espejismo-core/src/config/defaults.rs`,
  `crates/espejismo-core/src/config/mod.rs`, and the pacing logic in
  `crates/espejismo-core/src/protocol/framing.rs`.
- `git diff --check` passed. No cargo test or throughput benchmark applies
  because runtime behavior was not changed.
- Result: the configuration guide now distinguishes application-level
  outbound pacing, per-user aggregate bandwidth, and concurrency limits. No
  behavior regression is expected; numeric performance change is not
  applicable.
