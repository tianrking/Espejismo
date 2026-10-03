# Error codes documentation

## Findings and plan

The runtime does not define one stable application error-code enum: CLI and
server failures are contextual `anyhow` messages, network failures include OS
I/O errors, and the local proxy implements a subset of SOCKS5/HTTP statuses.
Therefore the reference documents observable protocol statuses and common
message patterns separately, and explicitly says free-form messages are not a
stable API. It also distinguishes local proxy request acceptance from remote
egress success, which can otherwise lead operators to misdiagnose a later
stream failure.

Added `docs/deployment/ERRORS.md` with a symptom/layer/action table, current
SOCKS5 and HTTP statuses, and links to existing validation, probe, logging, and
troubleshooting procedures. Linked it from the deployment troubleshooting guide.
No Rust code, protocol behavior, or project positioning changes.

## Expected impact

This documentation-only change should reduce time spent locating whether a
failure is in configuration, local proxy authentication, peer handshake,
network transport, or remote egress policy. There is no runtime, throughput, or
security behavior improvement claimed.

## Review and validation

- Cross-checked SOCKS5 method and reply values against
  `crates/espejismo-core/src/ingress/socks5.rs` and HTTP status responses against
  `crates/espejismo-core/src/ingress/http_proxy.rs`.
- Cross-checked example diagnostics against config validation, handshake, and
  egress policy error paths, and confirmed referenced deployment pages exist.
- Documentation-only change: Rust tests and throughput benchmarks are not
  applicable; no measured performance improvement is claimed.
