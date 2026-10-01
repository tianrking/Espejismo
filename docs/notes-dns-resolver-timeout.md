# DNS resolver timeout

## Findings and approach

Connection setup and egress used `tokio::net::lookup_host` without an application
deadline. A slow platform resolver could therefore hold a client connect or a
server relay task indefinitely before any socket connect timeout applied. The
client's Linux/macOS/Windows TUN route protection and both config diagnostics
also awaited lookups directly.

Tokio delegates hostname resolution to the platform resolver. A caller timeout
can bound how long the async operation waits, though it cannot forcibly stop an
already running OS resolver call. The implementation uses one shared 10 second
deadline and returns a contextual error; it does not add application retries,
since the system resolver already owns retry policy and layering retries could
multiply delays. This follows the async bounded-wait approach used in Rust
networking stacks while preserving Espejismo's existing TCP/yamux and egress
policy model.

## Changes and expected effect

- Added a shared socket-address resolver with a 10 second deadline.
- Routed core TCP connection setup, server TCP/UDP egress, client config checks,
  and platform TUN server-route lookups through it.
- Added a deterministic pending-future test proving a stalled resolution returns
  a timeout error promptly.

Expected improvement: worst-case async wait in these DNS paths is reduced from
unbounded to 10 seconds, after which existing error handling can release the
connection/relay task. Successful resolution behavior and returned address
ordering remain unchanged. No throughput change is expected.

## Verification

`cargo test -p espejismo-core -p espejismo-client -p espejismo-server --offline`
passed: 29 client tests, 126 core tests, 1 core doc test, and 17 server tests.
`cargo test --workspace --offline` passed those same suites and the yamux unit
suite, but its `window_update_deadlock` integration test could not bind its
local socket in this sandbox (`PermissionDenied`, OS error 1). This failure is
environmental and unrelated to DNS; the workspace command therefore exits
nonzero here. `cargo fmt --all -- --check` also reports existing formatting
differences in unrelated config/transport/server code; no workspace-wide
formatting changes were applied.
