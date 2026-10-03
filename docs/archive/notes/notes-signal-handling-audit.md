# Signal handling audit

## Findings and approach

- The local proxy previously selected on Ctrl-C and SIGTERM, then aborted its
  listener tasks. The remote listener loop had no signal branch and therefore
  relied on process-default termination.
- Neither binary implements SIGHUP reload or SIGUSR actions. Reload remains
  available through the authenticated admin API; adding a second reload path
  would complicate the small operations model and is outside this audit.
- Both service binaries now exit their main service loop on Ctrl-C/SIGINT or
  SIGTERM. SIGHUP and SIGUSR1/SIGUSR2 retain their operating-system default
  behavior; they are not interpreted as reload or status commands.
- This change preserves the project's positioning and protocol behavior. The
  expected benefit is consistent, predictable service-loop shutdown with no
  throughput effect.

## Experiment

- `cargo test -p espejismo-client -p espejismo-server` passed: client 29/29,
  remote 16/16; both benchmark binary and shared applicable unit targets also
  completed successfully. No performance benchmark applies to this lifecycle
  correctness change.
- `cargo fmt --all -- --check` reports pre-existing formatting differences in
  client tests, core admin tests, and remote tests. The modified remote main
  file was formatted directly; unrelated formatting was left untouched.
- No subprocess signal-delivery regression test was added: delivering process
  signals to the shared test runner can affect unrelated tests. The regression
  coverage here is the remote service loop's signal-select branch plus the
  existing test suite; manual signal smoke testing remains useful on each
  target OS.
