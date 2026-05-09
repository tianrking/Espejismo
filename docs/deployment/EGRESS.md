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
```

Rules:

- `deny_private_ips`: blocks literal private, loopback, link-local, and special
  IP targets.
- `allow_hosts`: optional host allowlist. Supports exact names and `*.example.com`.
- `block_hosts`: host blocklist. Block rules are evaluated before allow rules.
- `allow_ports`: optional port allowlist.
- `block_ports`: port blocklist. Block rules are evaluated before allow rules.

The policy validates literal IPs immediately. Domain names are validated as
names before dialing, and resolved TCP/UDP addresses are filtered again before
the remote endpoint connects or sends a datagram.
