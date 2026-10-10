# Config Reload Safety

## Findings and approach

- Reload is exposed through the authenticated admin `POST /reload` and
  `POST /apply` actions. There is no SIGHUP handler in either binary, so this
  change hardens the existing reload path rather than adding a second control
  path.
- The remote action parses, applies command-line logging overrides, and builds
  all `RemoteSettings` before mutating the live `RwLock`. Keep that boundary
  explicit: only a fully constructed candidate reaches the commit helper.
- The local action likewise parses and builds `LocalRuntime` before replacing
  the tunnel manager. The manager replacement itself and runtime snapshot are
  separate internal swaps; both currently have infallible operations after
  candidate construction. A future fallible manager preparation should happen
  before either swap or introduce a single shared commit boundary.
- This follows the established Rust service pattern of constructing a new
  immutable settings value and then swapping the live reference, while
  preserving Espejismo's small authenticated admin control plane and existing
  protocol/operational model (see `docs/POSITIONING.md`). No transport or
  configuration semantics are changed.

## Change and expected effect

- Added a named remote settings commit helper. Validation/build errors return
  before the live settings lock is changed; successful reloads replace the
  complete settings value in one write.
- Added regression tests for successful whole-value replacement and for an
  invalid candidate leaving the current users/settings untouched.
- Expected reliability effect: eliminate accidental future partial updates
  inside the remote commit path and make the failure invariant executable.
  This is a correctness change; no throughput gain is expected or claimed.

## Experiment

- `$HOME/.cargo/bin/cargo test -p espejismo-server` passed: 14 passed, 0
  failed (including both new reload safety tests).
- No performance benchmark is applicable to this correctness-only change.
