# DNS behavior and DoH documentation

## Findings and approach

The code uses `tokio::net::lookup_host` through the shared resolver in
`espejismo-core/src/dns.rs`. That resolver delegates to the platform resolver
and bounds the async wait to 10 seconds; it does not select a DNS provider or
offer DNS-over-HTTPS. Client `local.server` resolution and TUN route protection
run locally, while the remote resolves destination names for TCP/UDP egress.
SOCKS5 clients can request proxy-side destination resolution with the `h`
variant (`socks5h://`).

TUN `dns_servers` is separate: it supplies IP addresses to the OS DNS
configuration when optional DNS takeover is enabled. Platform implementations
use `resolvectl` on Linux, `networksetup` on macOS, and `netsh interface ipv4`
on Windows (IPv4 only). The saved DNS state is restored on orderly shutdown or
with route cleanup after interruption.

The documentation will state clearly that DoH URLs/options are unsupported and
explain the distinction between hostname resolution and OS DNS takeover. This
matches Espejismo's small configuration model and does not add a resolver or
protocol feature.

## Changes and expected effect

- Added `docs/deployment/DNS.md` as a concise guide to local/remote lookup,
  SOCKS5 remote resolution, the 10-second wait bound, opt-in TUN DNS takeover,
  supported OS mechanisms, and the absence of DoH support.
- Linked the guide from TUN and configuration documentation and clarified that
  `dns_servers` configures the OS DNS client.
- Expected effect: no runtime or performance change; users should be less likely
  to mistake TUN DNS server selection for built-in DoH or to expect all names to
  resolve on the same machine.

## Verification

Documentation-only review against `crates/espejismo-core/src/dns.rs`, the
client/server resolver call sites, and the Linux/macOS/Windows TUN DNS
implementations. Checked the changed Markdown links and configuration names
against the repository docs and code. No runtime behavior changed; no benchmark
or Rust test was applicable. No measurable performance change is expected.
