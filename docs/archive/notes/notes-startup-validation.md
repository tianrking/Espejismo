# Startup validation

## Findings and approach

The `--check-config` and `--doctor` paths already probe configured TCP listener
addresses, but ordinary startup did not use those probes. The server started
its admin task before binding its tunnel listener set, and the client started
admin before binding the SOCKS5 and HTTP listeners. A later bind failure could
therefore leave a partially started process, and admin/proxy address reuse was
only rejected by the explicit diagnostic command.

Startup now rejects admin/tunnel address reuse on the server and admin/proxy
reuse on the client, and binds the complete server tunnel listener set and all
client proxy listeners before spawning the admin or accept tasks. Socket bind
errors retain the precise address and OS cause from `bind_tcp_listener`.
This is a small operational reliability change; it does not alter the wire
protocol or the project's deployment model. The expected gain is earlier,
actionable failure with no partially started listeners when a configured port
is unavailable.

## Validation

`cargo test --offline -p espejismo-server -p espejismo-client` passed after the
changes: 28 client tests and 12 server tests passed, including new regression
tests for rejecting admin listener address reuse. No failures or behavioral
regressions were observed. This change has no performance claim; the expected
benefit is deterministic fail-fast startup and clearer port conflict errors.
