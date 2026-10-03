# TLS Certificate Validation Documentation

## Findings

- The primary Espejismo client/server tunnel does not use TLS or X.509
  certificates. It uses its native authenticated handshake and encrypted
  frames, consistent with the no-camouflage positioning in `POSITIONING.md`.
- `espejismo-server/src/http_chain.rs` uses rustls with the compiled
  `webpki-roots` set for `https://` upstream HTTP proxies, performs hostname
  validation and uses the endpoint host as the server name. There is no custom
  CA, client-certificate, or verification-disable configuration.
- The HTTP/2 underlay is cleartext prior-knowledge h2. TLS/ALPN for a public
  HTTPS endpoint belongs to an optional external reverse proxy.

## Change and rationale

Added `docs/deployment/TLS-CERTIFICATES.md` to explain where TLS certificate
validation does and does not occur, identify the trust and hostname behavior
for HTTPS egress proxies, and map typical failures to actionable checks.
Added links from egress and general troubleshooting documentation so operators
can find the explanation from the relevant workflow. This is documentation
only and preserves the project's protocol and deployment model.

Expected benefit: fewer misdirected certificate investigations and clearer
separation between tunnel authentication failures, egress proxy TLS failures,
and reverse-proxy certificate failures. This documentation change has no
runtime or throughput effect; a percentage improvement is not applicable.

## Verification

Reviewed the implementation in `crates/espejismo-server/src/http_chain.rs`,
the HTTP/2 underlay description in `docs/deployment/CONFIG.md`, and the
project positioning. Checked edited Markdown links and `git diff --check`.
No code changed, so Cargo tests are not applicable. No performance experiment
is applicable to this documentation-only change.
