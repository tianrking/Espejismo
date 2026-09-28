# Long transfer failure reporting

## Findings and reference

`docs/KNOWN_ISSUES.md` records intermittent 64 MiB de → jp downloads ending at
28–53 MiB with curl exit 18, on both public-IP and tailnet underlays. The
existing `encrypted_transport_with_yamux_mux_preserves_bulk_integrity` test
checks all 64 MiB through the encrypted yamux stack, but its in-memory duplex
has no packet loss, latency, or path MTU variation. This environment has no
de/jp endpoints or corresponding runtime logs, so the public route failure
cannot be reproduced here.

The relay used `tokio::select!` to read both directions. It treated a normal
EOF, any socket read error, and expiration of the per-read idle timer as the
same event: shut down the opposite write half and eventually return success.
That can turn a connection reset into an apparently clean but truncated HTTP
response. It fits curl exit 18, but does not prove the reported incidents had
this cause. The handler already logs relay errors, so propagating read errors
will make this distinction visible without changing the protocol or tunnel
positioning.

The references document points to `hashicorp/yamux` for stream window and
keepalive practices and `shadowsocks-rust` for async I/O patterns. This change
does not tune Yamux windows or introduce a new transport: it preserves the
existing TCP/yamux design and makes relay I/O failure handling distinguish
errors from EOF. An idle timeout retains the current half-close behavior.

## Change and expected effect

Changed `metered_idle_copy_bidirectional` to propagate read errors with the
failing relay direction in the error chain. Clean EOF and idle timeout continue
to half-close the other direction as before. Added a regression test that
injects a connection reset and verifies that it is returned as an error.

Expected throughput improvement: **0% claimed**. The intended benefit is
correct failure reporting and avoiding a success-shaped truncated response;
it does not increase transfer speed. Public de → jp reproduction and attribution
remain open until the same request is run on that route with peer logs.

## Verification

Command:

```sh
cargo test -p espejismo-core idle_copy_bidirectional --offline
```

Result: **3 passed, 0 failed** (read-error regression, idle-timeout behavior,
and half-close copy behavior). The first run exposed an overly narrow test
assertion because anyhow's default display omits the cause chain; the assertion
was corrected to check the full chain and the rerun passed.

The full `cargo test -p espejismo-core --offline` run passed the other 102
tests but the existing 64 MiB encrypted-yamux integrity case was still running
after 60 seconds. A focused run of that case showed the same behavior and was
interrupted. Therefore the full core suite is **not confirmed passing** in this
environment. This long-running test needs investigation before treating the
64 MiB loopback path as verified here.

The 64 MiB encrypted-yamux integrity test remains available as a separate
loopback regression, not a public-network reproduction. No before/after
throughput measurement was run because this is a correctness and diagnostics
change, not a performance optimization. No throughput or public-route
stability gain is claimed.
