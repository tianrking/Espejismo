# Connect timeout jitter coverage

## Findings and approach

The client records an underlay connect timeout as a lane failure, then waits
before the next on-demand connection attempt. Retry guidance in
`docs/research/REFERENCES.md` supports bounded randomized backoff to reduce
synchronized retries. The existing sampler chose an integer percentage, which
gave each percentage an equal chance even though the resulting millisecond
delays can have uneven representation after integer rounding.

## Changes and expected benefit

Sample the final integer-millisecond delay uniformly across the existing
80–120% bounds, retaining the same exponential schedule and 16 s cap. Rename
the lane spread regression to explicitly cover connect-timeout retries. This
does not change the protocol or connection deadline; it improves retry spread
uniformity without increasing the maximum wait. No throughput gain is expected.

## Verification

`$HOME/.cargo/bin/cargo test -p espejismo-client --offline` passed all 46
tests (0 failed, 0 ignored). The connect-timeout regression samples 256 retry
delays and checks the 80–120% bounds, at least 20 distinct millisecond slots,
and a mean between 1,900 and 2,100 ms for a nominal 2,000 ms delay. Existing
backoff tests also cover exponential growth, bounds across failure counts,
saturation, and the 16 s maximum. `rustfmt --edition 2021
crates/espejismo-client/src/tunnel.rs` and `git diff --check` passed. This is a
retry-correctness change, so a throughput benchmark does not apply; runtime
connection deadlines and maximum retry waits are unchanged.
