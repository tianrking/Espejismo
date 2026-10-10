# HTTP/2 priority dependency boundary tests

## Findings and approach

The HTTP/2 adapter delegates framing to `h2`. The repository reference list
points to mature protocol state-machine tests as the model for raw-wire
boundary coverage. Inspection of the resolved `h2` 0.4.16 source showed that
its PRIORITY handler currently logs `TODO: handle`; it does not keep a priority
tree or schedule DATA by dependency and weight. Implementing a separate tree
would add an unsupported behavior and is outside this test-focused change.

The existing in-memory tests cover self-dependency rejection, legal weight
endpoints, root and exclusive encodings. This change adds a cycle across two
idle streams and a 64-edge dependency chain, followed by a valid request. It
pins that these legal PRIORITY frames do not prevent connection use, without
claiming the library repairs cycles, enforces a depth cap, or allocates
bandwidth by weight. All input uses Tokio `duplex`; no loopback sockets are
needed and project positioning is unchanged.

## Expected impact

Correctness regression coverage only; no runtime behavior or performance
change is intended, so no throughput gain is claimed. The test will flag
future decoder or connection-state regressions for cyclic and deep PRIORITY
sequences. Weighted scheduling and an explicit tree depth limit remain
unimplemented in the dependency and are not tested as supported behavior.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core http2_priority
  -- --nocapture`: 6 passed, including the cycle plus 64-edge chain and
  subsequent request decode.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core`: 316 unit tests,
  1 config example test, 10 HTTP proxy tests, and 1 doctest passed; 1 existing
  loopback-bind test was ignored as required in the sandbox. No failures.
- No benchmark applies because this change only adds tests and documentation;
  runtime code is unchanged.
