# Deployment Quickstart Audit

## Scope and findings

Walked through `docs/deployment/QUICKSTART.md` against the installer,
`configs/examples/espejismo.toml`, the configuration guide, and the client and
server config validation paths.

- The installer extracts the full release package under `~/.espejismo`; its
  documented binary and config paths match `scripts/install.sh`.
- The example config includes `[[remote.users]]` with a PSK of its own. The
  quickstart previously instructed users to change `[shared].psk` without
  saying to change the remote user's PSK too. With the default user table
  present, those values must match for client authentication, otherwise
  `--probe-server` fails.
- The two binaries read role-specific sections from the same TOML. The client
  needs `[local].server` set to the reachable server address; the server uses
  `[remote].listen`. The example's local server address is loopback, so it
  cannot be left unchanged for a remote deployment.
- The example's egress allowlist permits the documented HTTP/HTTPS ports and
  denies private/special destination IPs, consistent with the safety guard in
  the quickstart.

## Change and expected result

Updated the Configure section to identify both PSK fields that must match,
explain the included default user table, distinguish client and server
addresses, and describe the single-key alternative (remove `remote.users`).
This should prevent a common first-run authentication failure and avoid trying
to connect to loopback on the client. No runtime behavior or performance is
changed; expected throughput change is 0%.

## Verification

- Checked the install paths and package naming against `scripts/install.sh`.
- Cross-checked role-specific config handling, required values, and remote user
  PSK matching against `docs/deployment/CONFIG.md` and the client/server
  validation code.
- Attempted to access the documented installer source with `curl`; this
  environment cannot resolve `raw.githubusercontent.com`, so a clean release
  download/install and live two-machine probe could not be performed.
- Documentation-only change: no cargo tests or throughput benchmark apply.
  `sh -n scripts/install.sh` and `git diff --check` are local checks.
