# Hot Reload Configuration Reference

## Findings and approach

- The admin guide already described the authenticated `/reload` and `/apply`
  paths and separated runtime settings from restart cases, but grouped supported
  fields into broad categories. Operators could not map a TOML key directly to
  the documented apply behavior.
- The client builds a replacement `LocalRuntime`; the remote builds a complete
  replacement `RemoteSettings`. The documented lists were checked against
  `build_runtime`, `build_remote_settings`, `RemoteSettings`, startup listener
  setup, and the `/reload`/`/apply` actions.
- Added exact TOML key paths and table wildcards to the deployment guide,
  including configuration captured during startup. Clarified that applying a
  setting does not renegotiate existing streams or recreate process-owned
  listeners/resources. This keeps the existing small authenticated admin
  control plane and does not change protocol behavior or project positioning.

## Change and expected effect

- Expanded the process-specific reload table in `docs/deployment/ADMIN.md` to
  identify supported and restart-required keys for both binaries.
- Expected benefit: operators can decide whether a config edit needs a reload
  or process restart without guessing from broad setting categories. This is a
  documentation-only usability improvement; no runtime or performance change
  is expected.

## Documentation check

- Cross-checked the listed fields against `crates/espejismo-client/src/main.rs`
  (`build_runtime`), `crates/espejismo-server/src/main.rs`
  (`build_remote_settings` and startup setup), and
  `crates/espejismo-core/src/config/types.rs`.
- Ran `git diff --check`; it passes. No cargo tests or throughput benchmark
  apply to this documentation-only change.
- Result: the guide now maps exact TOML keys to apply/restart behavior. No
  implementation behavior changed; no regression or throughput change is
  expected.
