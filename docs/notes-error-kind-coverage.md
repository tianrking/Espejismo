# Error kind classification branch coverage

## Analysis and plan

The stream failure classifier introduced in commit `55c3130` has three
operational outcomes: request and policy failures are `user_error`, timeouts
and sourced I/O errors are `network_error`, and unmatched failures are
`internal_error`. Its dedicated egress counter uses a separate message-marker
helper. Existing tests sampled only one egress marker and a directly wrapped
I/O error, leaving the marker alternatives and source-chain behavior under
specified.

Add focused unit coverage for all five egress markers, the negative egress
path (quota, timeout, and internal messages), and an I/O error nested under
anyhow context. This improves confidence that classification and the separate
egress counter predicate remain stable without changing runtime behavior or
the project's positioning. Expected throughput impact is zero: this only
changes tests, and classification still runs only while handling failures.

## Validation

- `$HOME/.cargo/bin/cargo test -p espejismo-server --offline`: passed, 23
  tests, 0 failures. The added cases exercise all egress marker alternatives,
  non-egress inputs, and traversal of a nested I/O error source chain; the
  existing test covers quota, timeout, direct I/O, and internal classification.
- No performance benchmark applies because production code and protocol
  behavior are unchanged.

Conclusion: classifier and egress-predicate regression coverage is expanded
with no observed server-suite regressions.
