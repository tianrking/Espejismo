# Graceful Configuration Reload Documentation

## Findings and approach

- Both binaries expose config changes through the authenticated admin `POST
  /reload` and `POST /apply` actions. There is no automatic file watcher or
  `SIGHUP` reload path.
- `/reload` rereads the original `--config`/`--config-base64` input and is
  unavailable without one. `/apply` accepts TOML in its request body and does
  not change the source for a later `/reload`. Startup CLI overrides are
  reapplied; the local client also reapplies its startup profile import.
- Each action builds a candidate before replacing runtime state. Remote applies
  replace `RemoteSettings`; local applies build a runtime and replace the
  tunnel manager. These paths do not recreate all resources captured at
  startup. Existing streams retain their current resources/settings.
- The deployment guide previously gave only broad runtime-managed categories.
  The update documents source semantics, CLI precedence, successful-apply
  behavior, remote and local runtime settings, startup-owned settings, and
  when active streams observe an update. This matches the small authenticated
  admin control plane and does not change Espejismo's protocol or operating
  model (`docs/POSITIONING.md`).

## Change and expected effect

- Expanded `docs/deployment/ADMIN.md` with reload/apply behavior and a
  role-specific table of runtime-updated versus restart-required settings.
- Expected benefit: operators can choose reload/apply or restart from the
  affected setting and understand which connections see the change. This is a
  documentation-only usability improvement; no runtime or performance change
  is expected, so there is no numeric performance claim.

## Experiment and documentation check

- Compared the documented endpoint behavior and updated/restart-required
  settings against `crates/espejismo-core/src/admin.rs`,
  `crates/espejismo-client/src/main.rs`, and
  `crates/espejismo-server/src/main.rs`.
- Confirmed the scope remains documentation-only and `git diff --check` passes.
  No cargo tests are applicable to this documentation-only change.
- Result: no implementation behavior changed; documentation covers the
  authenticated manual reload path, in-memory apply boundary, and restart
  cases. No regression or throughput change is expected.
