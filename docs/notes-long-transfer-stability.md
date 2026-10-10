# Long transfer stability notes

## Findings

The reported curl exit 18 means the HTTP response ended before its declared
length. The mux and encrypted frame transport do not impose a total stream
duration. Their idle timers are separate from active transfer duration. The
server TCP relay does treat an idle timeout on either copy direction as EOF and
shuts down the opposite socket's write half; this is a candidate to investigate
if a transfer pauses longer than `shared.idle_timeout_secs`, but current local
evidence does not establish it as the cause of the reported mid-transfer
disconnect.

The existing encrypted Yamux integrity test covered only 1 MiB, so it could not
exercise the reported transfer size. It now transfers 64 MiB through the
encrypted frame transport and Yamux stream, then checks every byte and clean
stream shutdown. Run it with:

```sh
cargo test -p espejismo-core encrypted_transport_with_yamux_mux_preserves_bulk_integrity
```

This is a loopback integrity regression, not a reproduction of the de -> jp
network failure: it has no packet loss, latency, or path MTU variation. No code
change to relay timeout, mux window, or pacing is justified without a failing
network reproduction or correlated logs. To finish root cause analysis, capture
both peers' stream failure reason and byte counters, effective
`shared.idle_timeout_secs` and mux mode, and repeat the same 64 MiB request over
the failing route while recording whether the failure is an idle timeout, mux
session end, or underlying TCP close.
