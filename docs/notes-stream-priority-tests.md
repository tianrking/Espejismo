# Stream priority fairness

## Research and plan

`docs/research/REFERENCES.md` identifies Yamux as the project's multiplexing reference and recommends learning its queue/window practices without changing the TCP/Yamux product direction. The project keeps its native authenticated encrypted tunnel and existing Interactive/Bulk classes. Inspection of `crates/espejismo-core/src/mux/native/pending.rs` found strict Interactive-first dequeueing: a continuously nonempty interactive queue could starve Bulk indefinitely.

Keep control frames first and Interactive preferred, but bound the preference: when both data classes are queued, send at most eight consecutive Interactive frames before one Bulk frame. When Bulk is not queued, Interactive remains unrestricted. This is a scheduling fairness bound, not a throughput claim; expected benefit is finite Bulk service latency under interactive load, at the cost of one Bulk slot per nine data frames in sustained contention.

## Implementation and evidence

- Added the bounded burst rule to `PendingFrames`.
- Added deterministic queue tests for Bulk service after eight Interactive frames, and Interactive preference when Bulk is not backlogged.
- Validation: `cargo test -p espejismo-core --offline` passed: 158 unit tests, 1 config example integration test, 4 HTTP proxy integration tests, and 1 doc test (164 total). The focused `cargo test -p espejismo-core native_pending_frames --offline` also passed all 3 queue tests, covering the bounded Bulk service point, Interactive preference without Bulk backlog, and queue limit.
- This is a correctness/fairness change; no throughput claim or benchmark is applicable.
