# DNS Behavior

Espejismo uses different DNS paths depending on where a name is resolved.

## Hostname resolution

The client resolves `local.server` on the client machine. The remote resolves
destination hostnames for relayed TCP and UDP traffic on the server machine.
These lookups use Tokio's platform resolver, so the operating system's resolver
configuration and policy apply. Espejismo does not select a public resolver,
implement DNS-over-HTTPS (DoH), or provide a DoH URL, bootstrap address, or
resolver-specific TLS settings.

Connection setup allows up to three platform lookup attempts with 100 ms and
250 ms delays between failures, within a 10 second overall async deadline. The
resolver call is bounded by Tokio's timeout; this cannot forcibly stop
platform resolver work that continues outside the async future. Successful
hostname results are cached briefly; failed lookups are not cached.

With SOCKS5, use `socks5h://` in applications such as curl when the destination
hostname should be sent through the proxy and resolved by the remote server.
With `socks5://`, the application may resolve the hostname locally before
connecting to the proxy. HTTP proxy requests use the destination authority and
are resolved by the remote relay. Direct connections and the configured
`local.server` endpoint are resolved locally by the client.

## TUN DNS takeover

TUN DNS takeover is an opt-in operating-system setting; it does not turn
Espejismo into a DNS or DoH resolver. Set `[local.tun.route].dns_enabled = true`
or pass `--tun-auto-dns` to apply the IP addresses in `dns_servers` (or
`--tun-dns`) to the host's DNS configuration while route takeover is active.
The operating system's DNS client then sends queries to those servers. Choose
addresses reachable through the tunnel and allowed by the remote egress policy.

The default DNS server list is `1.1.1.1` and `8.8.8.8`; takeover is disabled by
default. Linux uses `resolvectl`, macOS uses `networksetup`, and Windows uses
`netsh interface ipv4` and currently supports IPv4 DNS addresses only. On
normal shutdown Espejismo attempts to restore the prior DNS state. After a
forced termination, use `--tun-route-cleanup` to replay saved route and DNS
state. See [TUN mode](TUN.md) for platform details and recovery steps.

For a local proxy without system-wide changes, leave TUN DNS takeover disabled
and configure applications to use the SOCKS5 or HTTP proxy. See
[Configuration](CONFIG.md) for the TUN settings.
