# Graceful degradation under resource pressure

## Findings and approach

The remote server's per-listener `accept` task previously exited on every accept error. A transient `EMFILE`/`ENFILE` descriptor shortage or `ENOMEM` could therefore permanently stop that listener, even after resources became available. The service already has explicit connection and stream caps; those reject excess work cleanly, but they do not protect the accept loop from operating-system resource errors.

On Linux, recognized descriptor and memory exhaustion errors now wait 250 ms and retry the same listener. Windows low-memory errors and Rust's portable `OutOfMemory` kind receive the same treatment. Other accept errors still stop the listener as before, avoiding an endless loop on permanent failures. This follows the general resource-boundary discipline used by Tokio-based servers such as sing-box and shadowsocks-rust: bound work and recover from transient pressure while preserving explicit limits. It does not alter the protocol or product positioning.

Expected outcome: a temporary shortage may delay acceptance by 250 ms per retry rather than taking the listener permanently offline. No throughput improvement is expected; this is a resilience fix. Unit tests cover recognized resource errors and ensure permanent errors are not classified for retry.

## Experiment

`cargo test -p espejismo-server` passed: 16 server unit tests passed, 0 failed (including both new resource classification tests). No performance benchmark is applicable to this correctness/resilience change; the expected benefit is service availability after transient resource pressure, not throughput.
