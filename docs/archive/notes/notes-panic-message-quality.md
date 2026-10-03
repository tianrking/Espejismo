# Panic message quality

## Audit and scope

Audited explicit panic sites in the Rust crates, separating runtime invariants from test
assertions. The runtime panic sites needing better diagnostics were three empty `unreachable!()`
branches in tokio-yamux session polling and the `Closed` arm in `StreamHandle::drop`. The session
channels each retain an owning sender, and the stream arm is excluded by the preceding state guard;
these are internal invariants rather than input validation paths. Nearby crypto/configuration
`expect` calls already state their invariants. Remaining explicit `panic!` calls are test assertions.

## Plan, changes, and expected benefit

Keep control flow and protocol behavior unchanged. Add the session type, tracked stream count or
stream ID, and violated invariant to each runtime unreachable panic. Clarify the test-only session
closure expectation as well. If an invariant is broken in a future change, the panic now points to
the affected session/stream and the condition to inspect, reducing the need to reproduce under a
debugger. No throughput, memory, or operational behavior change is expected. The invariant-only
branches cannot be reached through the normal public API, so no artificial test hook was added;
the affected crate's complete unit suite was run to ensure the source changes compile and the
existing session/stream behavior remains covered.

## Validation

- `rustfmt --edition 2021 crates/tokio-yamux/src/session.rs crates/tokio-yamux/src/stream.rs`: passed.
- `$HOME/.cargo/bin/cargo test -p tokio-yamux --lib --offline`: passed, 23 tests, 0 failures.
- No performance measurement was run because panic text changes do not affect normal execution.

Conclusion: panic context improved for the identified invariant failures; all yamux library tests
passed and no behavior or performance change is expected.
