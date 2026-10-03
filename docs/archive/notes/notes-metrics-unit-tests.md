# Metrics Label Unit Tests

## Analysis and change

Commit `b31513f` added Prometheus label escaping for role, user, and failure
reason values. Its initial regression test checked role and user rendering,
but bundled those cases and did not test the escaping helper's exact output or
the failure-reason metric path. Failure reasons are sanitized before rendering,
so that path has a different expected behavior from raw role and user labels.

This change adds focused unit tests for:

- Exact backslash, quote, and line-feed escaping in `push_label_value`.
- Escaped role and user values in rendered metrics, including the absence of
  an injected physical line break.
- A failure reason containing special characters being sanitized before it
  appears in its metric label.

Expected result: the regression suite will fail if any Prometheus escape is
removed, if role/user rendering bypasses the helper, or if failure reasons
stop being sanitized. No runtime behavior, protocol, or data-path work changes;
the expected performance effect is zero.

## Experiment

- `cargo test -p espejismo-core --offline` — passed (109 passed, 0 failed;
  doc tests: 0). The three metrics tests passed.
- This is a correctness-only test addition; a throughput benchmark is not
  applicable and no performance improvement is claimed.
