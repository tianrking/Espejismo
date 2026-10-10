# Espejismo media kit

This page provides a concise, reusable introduction to Espejismo for writers,
reviewers, and community maintainers. Check the linked project documentation
before publishing technical details because supported features can change.

## Short description

Espejismo is a native Rust encrypted tunnel for routing private client traffic
through an authenticated remote server. It provides SOCKS5, HTTP proxy, and
optional TUN ingress with a deliberately small operating model: one client
binary, one server binary, and one TOML configuration file.

## Boilerplate

### One sentence

Espejismo is a native Rust encrypted tunnel with a small operating model and a
core protocol that does not impersonate other protocols.

### Short paragraph

Espejismo routes client traffic through an authenticated encrypted tunnel to a
remote egress server. The core protocol does not imitate TLS, HTTP, or QUIC;
optional WebSocket and HTTP/2 underlays use those transports as they are. The
project focuses on a small self-operated deployment: a client, a server, and
one TOML configuration file.

### 中文简介

Espejismo 是一款原生 Rust 加密隧道，通过经过认证的远端服务器转发客户
端流量。核心隧道协议不伪装成 TLS、HTTP 或 QUIC；可选的 WebSocket 和
HTTP/2 underlay 使用真实的对应传输。项目侧重于小型自运维部署：一个客
户端、一个服务端和一个 TOML 配置文件。

## Fact sheet

| Item | Description |
| --- | --- |
| Project | Espejismo |
| Category | Self-operated encrypted network tunnel |
| Language | Rust |
| Components | `espejismo-local` client and `espejismo-remote` server |
| Client ingress | SOCKS5, HTTP proxy, and optional native TUN |
| Core transport | Authenticated encrypted TCP tunnel with yamux multiplexing |
| Optional underlays | WebSocket and HTTP/2, using their normal protocol behavior |
| Configuration | TOML |
| License | MIT; see the repository `LICENSE` file |
| Source and releases | [GitHub repository](https://github.com/tianrking/Espejismo) |

For release-specific platform support and features, use the current README and
release notes rather than treating this summary as a compatibility promise.

## Technical description and claim boundaries

The core protocol uses X25519 session setup and XChaCha20-Poly1305 protected
frames, alongside handshake replay defenses and configurable padding. These
mechanisms reduce stable plaintext protocol markers; they do not make a
connection invisible or unidentifiable. The underlying transport, timing,
traffic volume, and deployment remain observable.

Describe WebSocket and HTTP/2 as optional transport underlays. Do not call them
camouflage, claim that Espejismo looks like a browser or a mainstream service,
or promise censorship resistance, anonymity, or undetectability. Espejismo is
not a multi-protocol suite and does not provide a hosted service or account
system.

For protocol details, see [project positioning](POSITIONING.md),
[protocol specification](PROTOCOL.md), and [brand usage](BRANDING.md).

## Name and media assets

Write the name as **Espejismo**, with this capitalization. Use lowercase
`espejismo-local` and `espejismo-remote` for the binary names. The repository
does not currently provide an approved graphical logo, icon, press screenshot,
or alternate wordmark. Use the plain-text name; do not create an unofficial
graphic and present it as project artwork. See [brand usage](BRANDING.md)
before adapting project materials.

The README contains a deployment topology diagram that may be linked as
technical context. It is explanatory documentation, not a logo or promotional
art asset.

## Links and contact

- Project overview and current release information: [README](../README.md).
- Setup and operational details: [quickstart](deployment/QUICKSTART.md).
- Support, issue reports, and public contact route: [support guide](SUPPORT.md).
- Security vulnerability reports: follow the private process in
  [SECURITY.md](../SECURITY.md), not a public issue.

There is no separate press email or guaranteed response time. Use the public
GitHub issue tracker for general project questions; do not include private
credentials, endpoints, or user traffic details.
