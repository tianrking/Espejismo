# Connection idle timeout review

## Findings and approach

The configured `shared.idle_timeout_secs` is passed to client and server stream relays. In
`metered_idle_copy_bidirectional`, both read deadlines are restarted after either direction
moves data. This makes the timeout an aggregate connection idle deadline: activity in either
direction keeps the relay alive, while a full idle interval ends the copy and shuts down the
opposite write half. Native mux sessions have a separate idle timeout that applies only when
there are no open streams. The vendored yamux reference recommends keepalive for peer liveness;
that is distinct from reclaiming an established application stream that has no payload activity.

This review leaves timeout semantics and config unchanged. It adds a regression test that
proves traffic refreshes the deadline and that the connection is reclaimed once traffic stops.
That protects existing resource bounds without adding config knobs or changing the project's
small operations model or authenticated encrypted transport.

Expected throughput change: 0%, since the relay implementation and timeout values are unchanged.
Expected operational benefit: retain the intended connection-idle reclamation behavior against
future regressions; no quantified resource reduction is claimed.

## Validation

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core idle_copy_bidirectional`: passed
  all 4 matching tests, including the new timeout refresh regression.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core -p espejismo-client
  -p espejismo-server`: passed client 29, core 125, server 17, config example 1,
  and core doctest 1; no failures.
- No performance benchmark was run: this is a correctness and resource-lifecycle test
  change with no runtime hot-path changes. Throughput is expected to remain unchanged.
