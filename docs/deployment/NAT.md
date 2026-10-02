# NAT Deployment

Espejismo uses an outbound TCP connection from `espejismo-local` to the
configured `local.server`. A client behind a home router, carrier-grade NAT
(CGNAT), or another outbound NAT usually needs no inbound mapping: the NAT
device tracks the outgoing connection and its replies. The client's SOCKS5
and HTTP proxy listeners are local application endpoints; they do not need to
be exposed through the client router.

The remote server must accept an incoming TCP connection on its configured
`remote.listen` port. If the server is behind a NAT router, configure a TCP
port-forward on that router to the server's private address and listening port.
Give the server a stable private address (for example, a DHCP reservation),
allow the same port in the host firewall, and allow it in any cloud firewall or
security group. Forward TCP only; the Espejismo underlay does not use UDP.

Example: the remote listens on `0.0.0.0:6690`, and the router forwards public
TCP port `6690` to `192.168.1.20:6690`. Set the client's `local.server` to the
router's public IP or a DNS name resolving to it, with port `6690`. If the
external port differs, forward that external TCP port to the configured
internal listener port and use the external port in `local.server`.

## NAT limitations

- A port-forward cannot create inbound reachability when the upstream ISP
  places the router behind CGNAT or another NAT that you cannot configure.
  Use a server with a public address or an operator-managed reachable relay.
- A changing public IP requires updating the DNS record or client endpoint.
  Dynamic DNS can update the name, but does not itself open a blocked port.
- Router loopback (hairpin) NAT varies. A client on the same LAN may not be
  able to reach the public endpoint through the router's public address. Test
  from an outside network, or use the server's private address for clients on
  that LAN if routing and firewall policy allow it.
- IPv6 may provide direct reachability without port mapping, but the server
  still needs an IPv6 listener and host/network firewall rules that permit the
  port. See [IPv6 deployment notes](IPV6.md).

## Verify reachability

On the server, confirm the process is listening on the intended interface and
port, then from a network outside the server's LAN run:

```bash
espejismo-local --config ./espejismo.toml --probe-server
```

The probe checks TCP connectivity and the Espejismo handshake. A successful
probe confirms that this client can reach and authenticate with that endpoint;
it does not validate access from every network or start the local proxy. If it
fails, check the listener bind address, router forwarding destination and
protocol, host/cloud firewalls, the public address or DNS answer, and whether
CGNAT prevents inbound connections. See [Troubleshooting](TROUBLESHOOTING.md).
