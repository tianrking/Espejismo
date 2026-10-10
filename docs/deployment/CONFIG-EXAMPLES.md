# Configuration Examples

Use this index to choose a starting point. Espejismo uses one TOML file for
both binaries: the remote reads `[shared]`, `[remote]`, `[logging]`, and
`[admin]`; the local reads `[shared]`, `[local]`, `[logging]`, and `[admin]`.
Each process ignores the other role's section, so one file can be copied to
both hosts. Replace example credentials before deployment.

## Maintained file

| Example | Best for | Location |
| --- | --- | --- |
| Full one-file configuration | A broad reference of supported settings and defaults | [`configs/examples/espejismo.toml`](../../configs/examples/espejismo.toml) |

The full file is also checked by the core crate's configuration doctest. To
check role-specific requirements and local listener availability, run
`espejismo-local --config espejismo.toml --check-config` and
`espejismo-remote --config espejismo.toml --check-config` on the corresponding
hosts.

## Deployment scenarios

The following complete scenarios are embedded in the configuration reference:

| Scenario | Use it when | Details |
| --- | --- | --- |
| Minimal server and client | Starting with the smallest practical shared config | [Minimal Server And Client](CONFIG.md#minimal-server-and-client) |
| One user with local SOCKS5 | Setting up the usual single-user VPS client | [One user with a local SOCKS5 proxy](CONFIG.md#one-user-with-a-local-socks5-proxy) |
| Multiple users with per-user limits | Sharing one remote while retaining per-user credentials and limits | [Multiple users with per-user limits](CONFIG.md#multiple-users-with-per-user-limits) |
| TUN client with route and DNS takeover | Routing system traffic through the tunnel | [TUN client with route and DNS takeover](CONFIG.md#tun-client-with-route-and-dns-takeover) |

The scenario configs are examples in the guide rather than separate files.
The TUN scenario needs platform-specific privileges and careful route
validation; see [Native TUN Mode](TUN.md) before enabling route takeover.

## References

- [Configuration reference](CONFIG.md) — parameters, defaults, and validation.
- [Deployment quickstart](QUICKSTART.md) — install and start both roles.
- [Authentication and key management](AUTHENTICATION.md) — PSK and user
  credential handling.
