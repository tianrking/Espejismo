# IPv6 Deployment Notes

Espejismo supports IPv6 on its TCP client connections and SOCKS5 destination
path. This does not enable IPv6 route takeover in TUN mode; see [Native TUN
Mode](TUN.md#support-matrix) for the current platform limits.

## Server listener

`remote.listen` is one numeric socket address. Use brackets around an IPv6
literal, for example:

```toml
[remote]
listen = "[2001:db8:1234::10]:6690"
```

This binds the server to that IPv6 address only. An IPv4 bind uses the usual
form, such as `0.0.0.0:6690`. The configuration does not provide separate IPv4
and IPv6 listener entries. Whether a wildcard IPv6 socket (`[::]:6690`) also
accepts IPv4-mapped connections depends on the operating system's socket
behavior; verify reachability from both address families instead of assuming
dual-stack behavior. Permit TCP port 6690 in the host firewall and provider
security rules for each family you intend to serve.

## Client server address

An IPv6 literal in `local.server` also requires brackets:

```toml
[local]
server = "[2001:db8:1234::10]:6690"
```

For a hostname, the client resolves addresses through the host resolver and
tries the returned TCP addresses in order. A hostname with both A and AAAA
records is useful when the client network can reach either family, but the DNS
records, server binds, routing, and firewall rules must agree. A successful DNS
lookup does not prove that the IPv6 route or remote listener is reachable.

## Proxy destinations and TUN limits

SOCKS5 supports IPv6 destinations, including IPv6 literals. For a remote
egress policy, check that `deny_private_ips`, host rules, and port rules allow
the intended destination. The server applies address checks to resolved
addresses as well as names.

TUN global route takeover currently handles IPv4 only on Linux, macOS, and
Windows. It does not capture general IPv6 system traffic, so a host may send
IPv6 traffic outside the tunnel while IPv4 follows the TUN route. Keep using
the SOCKS5/HTTP proxy for applications that need the tunnel, or disable IPv6
outside the tunnel using host-level network policy if that is required by your
deployment. Windows TUN DNS takeover currently accepts IPv4 DNS server
addresses only; see [DNS behavior](DNS.md).

The userspace TUN stack validates UDP checksums before exposing datagrams to
the relay. IPv4 accepts a zero UDP checksum as the protocol's "checksum not
provided" value; IPv6 requires a non-zero valid checksum.

## Deployment checks

1. Validate the config with `--check-config` on the binary that uses it.
2. Confirm the server is bound to the intended family and that the host and
   provider firewalls permit the tunnel TCP port.
3. From the client host, check that the configured literal or hostname is
   reachable over IPv6 (for example, with `--probe-server`).
4. Test the application path through SOCKS5 with an IPv6 destination. Test
   IPv4 separately if the deployment is intended to serve both families.

The checks above verify network reachability; they do not change Espejismo's
protocol or promise that every host network has working IPv6 connectivity.
