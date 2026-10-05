# TLS Configuration and Certificate Documentation

## Findings

- The native client-to-server tunnel has no TLS or X.509 configuration. Peer
  authentication and frame encryption belong to Espejismo's native protocol,
  consistent with `docs/POSITIONING.md`.
- `remote.egress.proxy` can use `https://` to connect to an upstream HTTP proxy
  over TLS. `espejismo-remote` validates that proxy's certificate against
  compiled Mozilla WebPKI roots and uses the endpoint identity for hostname
  validation; no custom root, client certificate, or verification bypass is
  configured.
- The HTTP/2 underlay is cleartext prior-knowledge h2. An optional external
  reverse proxy may terminate public TLS and owns its own certificate lifecycle.
- Existing deployment documentation already described certificate validation
  and troubleshooting, but did not clearly enumerate absent Espejismo TLS
  options or say who provisions, renews, and reloads certificates.

## Change and rationale

Expanded `docs/deployment/TLS-CERTIFICATES.md` to make the configuration
boundary explicit, identify the certificate owner for both HTTPS egress proxies
and external reverse proxies, and clarify that `remote.egress.proxy` is the
only Espejismo setting selecting an HTTPS connection. Updated certificate
identity guidance to cover either DNS names or IP addresses in Subject
Alternative Name, matching the rustls server-name validation path.

Expected benefit: operators can determine where to configure TLS and where to
renew certificates without looking for nonexistent tunnel certificate fields.
This documentation-only change has no runtime or throughput effect; percentage
improvement is not applicable. It preserves the no-camouflage positioning and
the small deployment model.

## Verification

Reviewed `crates/espejismo-server/src/http_chain.rs`, the egress proxy section
of `docs/deployment/CONFIG.md`, the HTTP/2 underlay section, and
`docs/POSITIONING.md`. Confirmed relative links and headings in the edited
document and ran `git diff --check`. No code changed, so Cargo tests and a
performance benchmark are not applicable to this documentation-only change.
