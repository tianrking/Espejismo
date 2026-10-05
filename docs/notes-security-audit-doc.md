# Security Documentation Audit

## Scope and findings

Audited the root `SECURITY.md` reporting policy and the security index,
authentication guide, admin guide, configuration behavior, and proxy ingress
defaults/implementations. The vulnerability-reporting advice matches the
repository's stated process and makes no unsupported response-time promise.
The admin guidance matches the implementation: `/healthz` is unauthenticated,
other routes require the configured token, non-loopback admin binds require a
token, and an unset token leaves a loopback listener unauthenticated.

Found one useful omission in the authentication guide: local proxy credentials
could be mistaken for protection of the local connection or confused with the
tunnel PSK. In practice `local.auth` is optional, the SOCKS5 and HTTP proxy
listeners default to loopback, SOCKS5 username/password and HTTP Basic carry
credentials without transport encryption, and these credentials do not
authenticate the remote tunnel peer.

## Change and expected benefit

Added a local-proxy-credentials section to
`docs/deployment/AUTHENTICATION.md` describing those defaults and boundaries,
and advising operators to retain loopback binding or restrict any reachable
listener. This is documentation only: it does not change networking,
authentication, protocol behavior, or the project's positioning. Expected
benefit is fewer deployment mistakes and clearer separation of local proxy
access credentials from tunnel authentication; no quantitative security gain
is claimed.

## Verification

Cross-checked the statements against `config/defaults.rs` and
`config/types.rs` (loopback defaults, auth unset), `config/mod.rs` (listener
validation and config permission warning), `ingress/socks5.rs`,
`ingress/http_proxy.rs`, and `admin.rs`. Verified the added section's links and
terminology against the adjacent authentication and admin documentation.
This is a documentation-only change, so no build or test was run. No
performance experiment applies.
