# Config Reload Regression Tests

## Findings and approach

The remote admin reload and apply actions fully parse and build a candidate
`RemoteSettings` before calling `replace_remote_settings`, which swaps the
complete value under one write lock. A parse or validation failure therefore
returns before the live value can change. The existing regression tests
exercised those boundaries, but the successful swap checked only user count
and failure preservation checked only the number of users; unrelated partial
updates could go unnoticed.

Keep the existing atomic replacement design and strengthen the regression
assertions across independently configured fields. This preserves the current
authenticated admin reload path and operational model without changing
configuration or protocol behavior.

## Change and expected effect

- Successful replacement now changes and verifies user count, idle timeout,
  and maximum streams together, ensuring the installed value includes the
  complete candidate rather than only one visible field.
- Invalid settings construction now verifies all three corresponding live
  values remain unchanged.
- Expected effect: stronger regression detection for partial or premature
  mutation in reload. This correctness-only test change has no runtime or
  throughput impact.

## Experiment

- `cargo test -p espejismo-server` passed: 21 tests passed, 0 failed. The
  successful whole-value replacement test verifies changed user count, idle
  timeout, and maximum streams; the invalid-candidate test verifies those live
  fields remain unchanged after candidate construction fails.
- No throughput benchmark applies to this correctness-only change.
