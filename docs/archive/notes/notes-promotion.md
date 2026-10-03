# Promotion Draft Notes

## Scope and source review

The referenced `promotion-plan.md` is not present in the repository, and the task brief ends after “2026-09-29 已按用户要求修订:”. This draft therefore follows the repository's current product description in `docs/POSITIONING.md`, `README.md`, and `docs/deployment/QUICKSTART.md`. It is an internal copy draft only; nothing has been published or sent.

## Copy direction

Lead with the project's defining choice: a native Rust encrypted tunnel that does not impersonate TLS, QUIC, or another protocol. Explain the small operating model (one client, one server, one TOML config) and link technical claims to the protocol and deployment documentation. Avoid claims of invisibility, censorship circumvention guarantees, universal security, or unmeasured speed. Keep the language factual and invite readers to inspect the protocol and limitations.

## Draft promotional copy

### Short description

Espejismo is a native Rust encrypted tunnel for private client traffic to a remote egress server. It avoids protocol impersonation and keeps setup to a client, a server, and one TOML config. Supports SOCKS5, HTTP proxy, and native TUN; read the protocol and deployment docs before trying it.

### Short social post

Introducing Espejismo: a native Rust encrypted tunnel that doesn't pretend to be TLS or QUIC. One client, one server, one TOML config, with SOCKS5, HTTP proxy, and native TUN support. No invisibility promises—just an auditable protocol and a deliberately small operating model. Explore the code and docs: https://github.com/tianrking/Espejismo

### Longer project introduction

Espejismo is a small, self-hosted encrypted tunnel for sending client traffic through a remote egress server. It uses authenticated encryption and does not imitate TLS, QUIC, or another familiar protocol. The operating model is intentionally compact: a client binary, a server binary, and one TOML configuration file. Client access is available through SOCKS5, HTTP proxy, or native TUN. The project does not claim invisibility; endpoint addresses, timing, traffic volume, and deployment choices remain observable. Start with the [project overview](../README.md), [protocol specification](PROTOCOL.md), and [quickstart](deployment/QUICKSTART.md).

## Pre-publication checklist

- Confirm the intended channels, audience, language, and final copy against the missing `promotion-plan.md` or its revised instructions.
- Verify the advertised release version, supported platforms, and feature availability against the release artifacts and docs at publication time.
- Keep the no-camouflage and no-invisibility-claim language intact; do not imply that encrypted traffic is undetectable or that a particular network outcome is guaranteed.
- Link to the repository, protocol specification, quickstart, and responsible-use guidance where space allows.
- Obtain separate user authorization before posting or otherwise sending any material externally.

## Expected impact

This is documentation-only work. It makes the project's audience, differentiator, setup model, limitations, and next steps easier to communicate consistently. No performance or adoption gain is claimed or measured.
