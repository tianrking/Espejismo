# Users, Quotas, And Bandwidth Limits

`espejismo-remote` can authenticate multiple independent users. Each user has a
separate PSK, optional rolling byte quota, and optional aggregate relay
bandwidth limit.

```toml
[[remote.users]]
name = "alice"
psk = "change-me-alice-secret"

[remote.users.quota]
bytes = 536870912
window_secs = 86400

[remote.users.bandwidth]
bytes_per_sec = 1048576

[[remote.users]]
name = "bob"
psk = "change-me-bob-secret"

[remote.users.quota]
bytes = 1073741824
window_secs = 86400
```

`quota.bytes` counts relay bytes in both directions for TCP streams and UDP
datagrams. When a user exceeds the current rolling window, new logical streams
are rejected and active streams stop at the next accounted chunk.

`quota.window_secs` defaults to 86400. Omitting `quota.bytes` disables quota for
that user.

`bandwidth.bytes_per_sec` applies to the user's aggregate relay traffic. It is a
portable userspace limiter, so it works on Linux, macOS, and Windows without
kernel-specific socket telemetry. Omitting it disables bandwidth limiting for
that user.

If no `[[remote.users]]` entries are configured, the server uses `shared.psk`
or `--psk` as a single fallback user named `default`, with no quota or bandwidth
limit.
