# CSP response boundary tests

## Change and rationale

The admin API emits JSON, Prometheus text, and plain-text responses; it has no
browser UI or content that needs script execution. Its shared response writer
now attaches the fixed policy `default-src 'none'; object-src 'none'; base-uri
'none'; frame-ancestors 'none'; form-action 'none'` to every response, including
health, authorization failures, capacity errors, and normal API responses.
Because the policy is a server constant, request-supplied CSP headers cannot
weaken or replace it. The response remains an API response and this adds no
new dependencies or transport behavior.

Reference review: `docs/research/REFERENCES.md` is oriented to transport and
protocol implementations, which do not apply to this HTTP response-header
boundary. This change follows the repository's existing focused, in-memory
admin protocol tests and does not alter Espejismo's positioning.

Expected effect: no throughput change; browser handling of any API response is
restricted to a no-content policy, reducing the impact if a response is ever
rendered in a browser context.

## Verification

- `$HOME/.cargo/bin/cargo test -p espejismo-core admin::tests --offline` passed:
  12 passed, 0 failed. The CSP test checks health (200), unauthenticated API
  (401), authenticated status (200), unknown route (404), and wrong method
  (405), verifies exactly one fixed CSP header, and attempts to weaken it with
  a request header. The client-capacity test also checks the 503 response.
- `$HOME/.cargo/bin/cargo test -p espejismo-core --offline` passed: 257 unit
  tests passed, 1 ignored (the existing loopback-bind test), 10 integration
  tests passed, and 1 doctest passed. No CSP regressions or other failures.
- `cargo fmt --all -- --check` reports formatting differences in numerous
  unrelated existing files. The changed Rust source was formatted directly
  with `rustfmt --edition 2021`.
