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

- `deny_private_ips`: blocks literal private, loopback, link-local, and special
  IP targets.
- `allow_hosts`: optional host allowlist. Supports exact names and `*.example.com`.
- `block_hosts`: host blocklist. Block rules are evaluated before allow rules.
- `allow_ports`: optional port allowlist.
- `block_ports`: port blocklist. Block rules are evaluated before allow rules.
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

Proxy behavior:

| Proxy URL | TCP egress | UDP egress | Auth |
| --- | --- | --- | --- |
| `socks://...` | SOCKS5 CONNECT | SOCKS5 UDP ASSOCIATE | no-auth or username/password |
| `socks5://...` | SOCKS5 CONNECT | SOCKS5 UDP ASSOCIATE | no-auth or username/password |
| `socks4://...` | SOCKS4 CONNECT to IPv4 literal targets | no | optional user id |
| `socks4a://...` | SOCKS4a CONNECT with remote domain resolution | no | optional user id |
| `http://...` | HTTP CONNECT over plain TCP | no | optional Basic auth |
| `https://...` | HTTP CONNECT inside TLS to the proxy | no | optional Basic auth |

HTTP and HTTPS proxy URLs describe the connection to the upstream proxy itself.
They are different from tunneling an HTTPS destination such as
`example.com:443`, which works through either plain `http://` CONNECT proxies or
TLS-protected `https://` CONNECT proxies.
