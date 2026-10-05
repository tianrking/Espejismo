# Signal handling documentation

## Scope and rationale

Document the process signals that the local and remote service binaries
actually handle, including platform differences and the boundary between
prompt process shutdown and graceful connection draining. This is a
documentation-only change: both binaries already select on Ctrl-C/SIGINT and,
on Unix, SIGTERM. The shutdown and restart guide previously described the
overall effect but did not enumerate unhandled signals or state the Windows
behavior explicitly.

The deployment guide now records that the first handled signal exits the
service loop; the local aborts and joins listener tasks, while the remote
stops accepting and exits without joining active peer handlers. SIGHUP and
SIGUSR1/SIGUSR2 have no application-defined reload or status action, and
SIGKILL/SIGSTOP cannot be handled. Runtime configuration changes remain the
authenticated admin API's responsibility. This preserves the project's small
operations model and adds no process or protocol behavior.

Expected improvement: operators can choose a supported stop signal and
understand restart impact without inferring behavior from implementation. No
runtime or throughput change is expected (0% by design).

## Experiment

- Documentation-only change; no benchmark is applicable.
- Verified the descriptions against `shutdown_signal()` in both
  `crates/espejismo-client/src/main.rs` and
  `crates/espejismo-server/src/main.rs`, and their service-loop shutdown
  branches.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-client -p espejismo-server`
  passed: client 31/31 and server 19/19 unit tests; the benchmark target's
  zero-test harness also completed successfully.
- Conclusion: documented behavior matches the source inspection; runtime
  behavior is unchanged and there was no test regression.
