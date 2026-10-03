# Default configuration tuning guidance

## Audit and change

`docs/deployment/CONFIG.md` already listed omitted-field defaults, but tuning
advice was distributed across field descriptions and examples. Added a concise
goal-based guide covering throughput, latency and memory, rate limits, replay
tolerance, TCP, TUN routing, egress policy, stealth shaping, port hopping, and
operations. Also clarified that optional credentials and endpoints remain
unset and that recommendations should be measured on the deployment path.

Defaults and field behavior were cross-checked against the configuration
reference and the schema/default implementations in
`crates/espejismo-core/src/config/types.rs` and
`crates/espejismo-core/src/config/defaults.rs`. No runtime setting, parser
behavior, protocol behavior, or project positioning changed.

## Expected benefit

Operators can map common goals to the relevant settings without treating larger
buffers or more lanes as universal improvements. No runtime or throughput gain
is expected from this documentation-only change; the intended improvement is
faster, safer configuration decisions.

## Verification

- Checked the tuning guidance against the documented defaults and implementation
  semantics, including disabled route/admin features, `0` uncapped pacing, and
  peer-shared handshake-window and port-hopping settings.
- No tests or performance experiment apply to this documentation-only change.
