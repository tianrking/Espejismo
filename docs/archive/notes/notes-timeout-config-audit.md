# Timeout configuration audit

## Scope and findings

Reviewed application runtime deadlines under `crates/espejismo-{core,client,server}/src`,
the TOML config/defaults, and the deployment configuration reference. Existing user-facing
timeouts are already configured for handshake (`remote.handshake_timeout_ms`), stream idle
(`shared.idle_timeout_secs`), native mux idle/drain, fallback HTTP probing, TUN UDP replies,
TCP keepalive, TCP user timeout, and user quota windows. They have defaults and are listed in
`docs/deployment/CONFIG.md` (TUN UDP is also described in `docs/deployment/TUN.md`).

The remaining fixed runtime deadlines are mostly local protocol/resource bounds, rather than
transport tuning knobs:

| Area | Fixed value | Reason / disposition |
| --- | ---: | --- |
| Admin API header and body reads | 15 s each | Bounds stalled local control-plane clients; documented next to constants. |
| HTTP proxy header read | 15 s per read | Bounds stalled local proxy requests while permitting incremental headers; documented next to constant. |
| Server stream permit wait | 15 s | Bounds task occupancy during global stream-limit saturation; documented next to constant. |
| Server tunnel request read | 15 s | Small protocol request; avoids holding authenticated peers indefinitely; documented next to constant. |
| Client lane connect/handshake | 30 s | Fixed ceiling for a full lane setup (DNS/TCP/handshake); documented next to constant. |
| TUN stream open | 10 s | Prevents packets from queuing behind unavailable tunnel capacity; documented next to constant. |
| Egress UDP reply / SOCKS5 UDP reply | 10 s maximum | Bounds one datagram transaction even if stream idle timeout is much longer; documented at cap. |
| Native mux struct fallback | 300 s idle / 30 s drain | Mirrors TOML defaults for direct library construction; documented at fallback. |
| HTTP request body copy grace | 1 s | Small eager-copy window for already-buffered fixed-length request bodies, not a request deadline. |
| Update/diagnostic probes | 10 s / 3 s | Best-effort CLI operations; not part of established tunnel traffic. |
| Reconnect backoff | 500 ms to 16 s (jittered) | Retry pacing rather than an I/O timeout; intentionally algorithmic. |

Tokio-yamux also has a 30 s keepalive timeout and a 10 s write safety timeout. These are
upstream-compatible mux behavior constants in the vendored `tokio-yamux` crate; changing them
would alter the mux protocol's failure detection for both peers, so they remain library policy.
The `tokio-yamux` keepalive interval and write timeout are represented in its config, but
Espejismo currently constructs yamux with library defaults.

## Plan and expected effect

This pass keeps the TOML schema stable. Promoting each local request-boundary timeout would
add low-value knobs to a deliberately small operational model and require deciding whether
each setting is local-only, shared, or peer-negotiated. Fixed resource guards now state their
purpose in code; the already configurable transport/session timeouts remain the tuning surface.
Expected throughput change is 0%: no timeout values or execution paths changed. The expected
operational benefit is clearer maintenance and fewer accidental edits that remove protection
against stalled peers or saturated pools; no numeric reliability gain is claimed without field
measurements.

## Validation

- `cargo test --offline -p espejismo-core -p espejismo-client -p espejismo-server` passed:
  client 27, core 119, server 11, and core doctest 1; no failures.
- `cargo test --workspace --offline` passed the same tests and the 23 `tokio-yamux` unit
  tests, but its `tokio-yamux` integration test `one_way_bulk_transfer_exceeding_window`
  could not bind its socket (`PermissionDenied: Operation not permitted`) under this sandbox.
  This test does not exercise changed code. No timeout values or runtime behavior changed, so
  expected throughput impact remains 0% and there is no behavior regression introduced.

## Follow-up decisions

- Consider exposing admin and local proxy read deadlines only if deployments report a concrete
  need; admin is local control-plane traffic and proxy ingress is client-local, so neither should
  silently become a peer-shared setting.
- Review the vendored yamux default configuration path if configurable keepalive/write failure
  detection becomes a requirement.
- Most timeout-looking literals in tests and benchmark helpers are harness watchdogs, not runtime
  settings, and are intentionally excluded from this audit's configuration surface.
