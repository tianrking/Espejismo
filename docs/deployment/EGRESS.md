# Egress Policy

`espejismo-remote` validates every requested outbound target before dialing.
Rules live under `[remote.egress]`.

```toml
[remote.egress]
deny_private_ips = true
allow_hosts = []
block_hosts = ["metadata.google.internal", "169.254.169.254"]
allow_ports = [80, 443]
block_ports = [25]
proxy = "socks5://user:pass@127.0.0.1:1080"
# proxy = "socks4a://127.0.0.1:1080"
# proxy = "http://user:pass@127.0.0.1:8080"
# proxy = "https://user:pass@proxy.example.com:8443"
# socks5_proxy = "127.0.0.1:1080" # legacy alias
```

Rules:

- `deny_private_ips`: blocks private, loopback, link-local, and special IP
  targets, including IPv4-mapped IPv6 literals according to their embedded
  IPv4 address. This also includes shared-use, benchmarking, documentation,
  multicast, and reserved ranges. For direct egress, every resolved destination
  IP is checked too, so a hostname resolving to a prohibited address is rejected.
  The setting defaults to `false`.
- `allow_hosts`: optional host allowlist. An empty list imposes no host
  restriction. Entries match case-insensitively and may be exact names or
  `*.example.com` patterns. A wildcard pattern matches both `example.com` and
  its subdomains (for example, `api.example.com`).
- `block_hosts`: optional host blocklist, with the same matching syntax as
  `allow_hosts`. Matching is case-insensitive; `*.example.com` blocks the
  apex and subdomains at label boundaries, but not names such as
  `badexample.com` or `example.com.evil`. A matching block rule takes
  precedence over the host allowlist.
- `allow_ports`: optional port allowlist. An empty list imposes no port
  restriction.
- `block_ports`: optional port blocklist. A blocked port takes precedence over
  the port allowlist.
- `proxy`: optional upstream proxy for server-side egress chaining. Supported
  forms are `socks://host:port`, `socks4://host:port`,
  `socks4a://host:port`, `socks5://host:port`,
  `socks5://user:pass@host:port`, `http://host:port`,
  `http://user:pass@host:port`, `https://host:port`, and
  `https://user:pass@host:port`.
- `socks5_proxy`: legacy alias for a no-auth SOCKS5 upstream. Prefer `proxy`
  for new deployments.

The policy validates literal IPs immediately. Domain names are validated as
names before dialing, and resolved direct TCP/UDP addresses are filtered again
before the remote endpoint connects or sends a datagram.

## Rule evaluation and examples

For each requested `host:port`, Espejismo checks the host blocklist, host
allowlist (when non-empty), port blocklist, port allowlist (when non-empty),
and then private/special IP policy for literal IP targets. Any failed check
rejects the request. For direct connections, resolved IP addresses are checked
against `deny_private_ips` and the port lists as a second boundary; hostname
lists match the requested host name and are not re-evaluated against resolved
addresses.

An allowlist is the narrowest way to define a small permitted set. For example,
this permits only HTTPS to the named service and its subdomains, while
explicitly blocking one subdomain:

```toml
[remote.egress]
deny_private_ips = true
allow_hosts = ["*.example.com"]
block_hosts = ["admin.example.com"]
allow_ports = [443]
```

Here `example.com:443` and `api.example.com:443` are permitted, but
`admin.example.com:443`, `api.example.com:80`, and `example.net:443` are
rejected. An empty `allow_hosts` or `allow_ports` list means unrestricted for
that dimension; block lists remain active independently. Review the effective
policy together with the upstream-proxy notes below when chaining egress.

Proxy behavior:

| Proxy URL | TCP egress | UDP egress | Auth |
| --- | --- | --- | --- |
| `socks://...` | SOCKS5 CONNECT | SOCKS5 UDP ASSOCIATE | no-auth or username/password |
| `socks5://...` | SOCKS5 CONNECT | SOCKS5 UDP ASSOCIATE | no-auth or username/password |
| `socks4://...` | SOCKS4 CONNECT to IPv4 literal targets | no | optional user id |
| `socks4a://...` | SOCKS4a CONNECT with remote domain resolution | no | optional user id |
| `http://...` | HTTP CONNECT over plain TCP | no | optional Basic auth |
| `https://...` | HTTP CONNECT inside TLS to the proxy | no | optional Basic auth |

For TCP, the remote sends the requested `host:port` to the upstream proxy:
SOCKS5 uses a domain-name CONNECT address, SOCKS4a sends the domain in its
extension format, SOCKS4 accepts only an IPv4 literal, and HTTP(S) sends an
HTTP/1.1 CONNECT authority. SOCKS5 therefore lets the upstream proxy resolve
domain targets; SOCKS4a does as well. SOCKS4 does not resolve names. These
chains do not perform local destination DNS resolution before proxying. The
remote still checks the requested hostname and port against `block_hosts`,
`allow_hosts`, `block_ports`, and `allow_ports`, and rejects private/special IP
literals when configured. Since the upstream resolves domain names, the remote
cannot apply `deny_private_ips` to the resulting destination address on a
chained request; enforce that restriction on the upstream proxy as well.

SOCKS5 offers no-authentication and, when credentials are configured,
username/password authentication. The upstream selects one offered method;
other SOCKS5 methods are unsupported. SOCKS4's optional user ID is not a
password-based authentication mechanism. HTTP(S) proxy credentials use Basic
authentication in `Proxy-Authorization`; use `https://` when that credential
must be protected on the connection to the proxy. HTTPS proxy TLS certificate
and hostname verification are enabled.

UDP is available only through SOCKS5 UDP ASSOCIATE. SOCKS4, SOCKS4a, HTTP, and
HTTPS chains cannot carry UDP; this includes SOCKS5's UDP relay being separate
from its TCP CONNECT stream. Without an upstream proxy, the remote resolves
the destination and applies resolved-address egress checks before direct TCP
or UDP traffic.

HTTP and HTTPS proxy URLs describe the connection to the upstream proxy itself.
They are different from tunneling an HTTPS destination such as
`example.com:443`, which works through either plain `http://` CONNECT proxies or
TLS-protected `https://` CONNECT proxies.

Espejismo validates HTTPS proxy certificates against its bundled Mozilla
WebPKI roots. Hostname matching and certificate troubleshooting are described
in [TLS Certificates](TLS-CERTIFICATES.md).
The remote shares Rustls session state across HTTPS proxy connections, allowing
later connections to resume when the proxy issues and accepts TLS tickets.
