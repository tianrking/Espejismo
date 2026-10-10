# Admin authentication regression tests

## Change and expected benefit

The admin handler already gates every route except `GET /healthz` when a token
is configured. Existing tests checked the token predicate and health-probe
predicate separately, but did not exercise HTTP responses or prove a denied
administrative request could not invoke its callback. Added handler-level tests
using Tokio in-memory duplex streams. This keeps coverage deterministic and
works in restricted environments where opening loopback sockets is prohibited.
The expected improvement is regression detection for missing/wrong credentials,
the public health exception, and protection of `/apply` side effects; there is
no runtime behavior or performance change.

Updated the admin endpoint guide to state that a configured token is checked
before request bodies are read or administrative actions run.

## Verification

- `cargo test --offline -p espejismo-core admin::tests`: 7 passed, 0 failed.
  The added HTTP-handler tests cover unauthenticated `/status`, `/connections`,
  `/metrics`, `/apply`; wrong bearer and Basic credentials; public `/healthz`;
  denial before the apply callback; and successful authenticated apply.
- First test implementation attempted loopback TCP and was blocked by the
  sandbox (`PermissionDenied`). Switched test transport to in-memory duplex
  streams and reran successfully; production listener behavior is unchanged.
- `cargo test --offline -p espejismo-core`: 144 unit tests, 1 config example
  integration test, and 1 doctest passed; 0 failed.
- `cargo fmt --all -- --check` reports pre-existing formatting differences in
  unrelated files; formatted only `crates/espejismo-core/src/admin.rs`.

Conclusion: targeted auth regression tests pass. No performance claim is made.
