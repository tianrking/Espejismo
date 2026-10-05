# Error kind taxonomy

## Audit and plan

The server's mux stream failure metric previously classified failures by message
substring into `egress_denied`, `timeout`, `quota`, or `other`. This made the
monitoring vocabulary depend on wording and collapsed network failures and
unexpected internal failures into `other`.

Use a stable three-class vocabulary at the stream-observation boundary:

- `user_error`: request/policy rejection, including egress denial and quota.
- `network_error`: timeout or an error with an `io::Error` in its source chain.
- `internal_error`: an unrecognized failure, kept visible rather than silently
  mixed with expected network churn.

The dedicated `egress_denied_total` counter remains intact. This is a
classification-only change; protocol behavior, error propagation, and the
project's authenticated-encryption/no-camouflage positioning do not change.
Expected benefit: alerts can distinguish actionable request or policy problems,
transport instability, and unexpected server failures with three bounded labels.
No throughput impact is expected because classification runs only on failed
streams. The classification still uses known egress/quota message markers where
those paths currently return `anyhow` errors; a future typed-error migration can
remove that remaining text dependency.

## Validation

- Added regression coverage for egress and quota rejections, explicit timeouts,
  sourced I/O errors, and an unrecognized internal failure.
- `cargo test -p espejismo-server --offline`: passed, 19 tests including all five
  classification cases.
- `cargo test --workspace --offline`: client tests passed (31); core had one
  transient failure in the unrelated `transport::tests::idle_copy_bidirectional_refreshes_timeout_on_traffic` case, after which the run stalled in later transport tests and was interrupted. Re-running that exact core test alone passed (1 passed). The full workspace run is therefore not cleanly verified.

Conclusion: the directly affected server suite passes. The workspace run exposed
an unrelated timing-sensitive transport test failure that passed on isolated
rerun; no failure was reported in error classification.
