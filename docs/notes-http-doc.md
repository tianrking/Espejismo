# HTTP ingress documentation

## Findings and plan

The configuration reference listed `local.http_listen` and
`local.http_bulk_threshold_bytes`, but it did not describe the request forms,
local authentication behavior, parser limits, or lane classification in one
place. Source review found that the HTTP ingress is a local application proxy:
it handles `CONNECT` and absolute-form `http://` requests, rewrites ordinary
HTTP requests to origin form, and forwards traffic through the existing
encrypted tunnel. It does not turn the tunnel into an HTTP protocol endpoint.

Following the focused SOCKS5 deployment guide's structure, this change adds
`docs/deployment/HTTP.md` and links it from `docs/deployment/CONFIG.md`. The
guide records the listener default and exposure considerations, shared
`[local.auth]` Basic proxy auth, supported request forms, header size and read
timeout, request-body handling, egress-policy relationship, and bulk lane
rules. This is a documentation-only usability improvement; expected throughput
or latency change is 0%.

This follows the project's single-client/single-server operations model and
does not add protocol behavior or claim HTTP camouflage. The references and
positioning documents were reviewed as required; no reference-project code or
design is needed for a description of the existing ingress implementation.

## Verification

Compared statements against `config/defaults.rs`, `config/types.rs`,
`ingress/http_proxy.rs`, and `client/handler.rs`. Confirmed the defaults
(`127.0.0.1:6681`, 1 MiB), Basic auth challenge behavior, accepted request
forms, header limit/timeout, and the upload/download priority rules. Reviewed
the changed Markdown links and examples for consistency.

No runtime source changed, so no throughput benchmark or Rust test was run.
Conclusion: documentation matches the inspected implementation; no runtime
regression is introduced. Performance impact: 0% expected (documentation only).
