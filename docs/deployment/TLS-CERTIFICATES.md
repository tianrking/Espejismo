# TLS Certificates

Espejismo does not use TLS for its own client-to-server tunnel. Its tunnel
authenticates peers with the configured PSK (or user PSK) and encrypts frames
with its native protocol. Consequently, there is no Espejismo tunnel
certificate to install, renew, or trust.

TLS certificates are checked in these deployment cases:

- When `remote.egress.proxy` uses an `https://` URL, `espejismo-remote`
  establishes TLS to that upstream HTTP proxy before sending CONNECT.
- If you put the HTTP/2 underlay behind a reverse proxy for public HTTPS,
  that reverse proxy terminates TLS and owns the public certificate. The
  Espejismo HTTP/2 underlay itself uses cleartext prior-knowledge h2 on its
  configured TCP endpoint; see [Configuration](CONFIG.md#sharedunderlay).

## HTTPS egress proxy validation

For an HTTPS proxy, Espejismo validates the proxy's server certificate against
the Mozilla WebPKI root set compiled into the application. It also checks the
certificate for the proxy hostname and sends that hostname as the TLS server
name (SNI). The HTTPS proxy setting has no option to disable verification,
provide a custom CA bundle, or configure a client certificate. The URL
hostname must therefore match the certificate and chain to a trusted root.

Check the following when TLS connection to the proxy fails:

1. Confirm the proxy endpoint is written as `host:port`, and use a DNS name
   that appears in the proxy certificate's Subject Alternative Name. A bare
   IP address may not be accepted as a TLS server name by the current client.
2. Check the proxy certificate's validity dates and renew it if expired or not
   yet valid. Confirm the server sends intermediate certificates needed to
   build the chain.
3. Check that the certificate chains to a root in the bundled Mozilla WebPKI
   roots. A private or enterprise CA is not trusted by this configuration.
4. Check that the machine clock is accurate; certificate date checks depend on
   it.
5. Confirm the configured port reaches the TLS-enabled proxy listener, not a
   plaintext HTTP proxy or another service.

An invalid `host:port` authority can fail before TLS starts with an error like
`invalid HTTPS proxy TLS server name`. A certificate, hostname, or TLS protocol
problem appears during `TLS handshake with HTTPS proxy`. Once TLS succeeds,
proxy authentication and CONNECT failures are separate: a non-200 CONNECT
response is reported as `HTTP proxy CONNECT failed`.

The error context may not include a precise certificate failure reason. Check
the upstream proxy's TLS logs and certificate configuration; do not work around
validation by disabling it. For a private CA, use a proxy whose certificate
chains to a trusted public root or place a trusted TLS terminator in front of
it.

## Reverse proxy certificate troubleshooting

When public HTTPS terminates at a reverse proxy, inspect the certificate and
TLS logs there. Check that the public hostname matches the certificate, that
the certificate is current, and that the proxy forwards to the configured
HTTP/2 underlay using the expected cleartext h2 mode. Espejismo does not read
or validate the reverse proxy's certificate. A TLS alert or browser certificate
warning is therefore diagnosed at the reverse proxy, while an Espejismo
handshake error after TLS termination concerns the tunnel's own PSK/settings.

See [Troubleshooting](TROUBLESHOOTING.md) for general connection diagnostics
and [Egress Policy](EGRESS.md) for upstream proxy configuration.
