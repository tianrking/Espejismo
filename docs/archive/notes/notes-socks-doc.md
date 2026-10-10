# SOCKS5 documentation update

## Scope and findings

This is a documentation-only change. The client defaults `local.socks5_listen`
to `127.0.0.1:6680`; the listener handles SOCKS5 TCP CONNECT and UDP ASSOCIATE.
`local.auth` controls both SOCKS5 and HTTP proxy authentication, and UDP
fragmentation is rejected. UDP relay binds its ephemeral socket to loopback.
Remote egress policy still applies, and chained UDP requires a SOCKS5 upstream.
SOCKS hostname resolution depends on whether the application sends a domain or
resolves it locally first.

The existing deployment docs covered these details across CONFIG, DNS, and
EGRESS, but had no focused SOCKS5 ingress reference. The update adds that
reference and links it from the configuration guide. This follows the project's
small operations model and documents its existing protocol behavior without
adding protocols or changing the positioning in `docs/POSITIONING.md`.

## Change and expected benefit

Added `docs/deployment/SOCKS5.md` with bind address, exposure guidance,
authentication, TCP/UDP capabilities, fragmentation limitation, DNS behavior,
and an end-to-end client example. Added a link from `CONFIG.md`.

Expected result: reduce setup ambiguity and incorrect assumptions around UDP,
authentication, and hostname resolution. This is a usability/documentation
benefit; no throughput or latency change is expected (0%).

## Verification

Reviewed the descriptions against `config/defaults.rs`, `config/types.rs`,
`ingress/socks5.rs`, `client/handler.rs`, and the existing DNS and egress docs.
Documentation-only change: no runtime behavior changed. `cargo test -p
espejismo-core ingress::socks5` passed (6 passed), and
`cargo test --doc -p espejismo-core` passed (1 doctest). A full
`cargo test -p espejismo-core` run printed the test results but did not exit;
it was interrupted after waiting, so the full crate suite is not claimed as
passed. No observed failures; the targeted protocol and config doctest checks
passed.
