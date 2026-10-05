# Glossary audit

## Review and changes

Compared `GLOSSARY.md` with the maintained protocol specification,
`docs/POSITIONING.md`, `docs/ARCHITECTURE.md`, and implementation names in the
core framing and client tunnel modules.

- `Frame` now mentions the `TARGET` frame type. The protocol specification and
  `FrameType` enum retain it for legacy compatibility, while the current
  request-preface flow does not use it. Omitting it made the glossary's list
  sound exhaustive.
- `Lane` matches the client `TunnelLane` abstraction: a reusable authenticated
  physical connection with one mux session, which carries independent logical
  streams.
- `Underlay`, `UDP relay`, and `UDP underlay` preserve the documented boundary:
  production transport uses TCP-based underlays, SOCKS5 UDP relay is carried
  over the tunnel, and UDP underlay primitives remain experimental.
- `Obfuscation profile`, `stealth`, and `Authenticated encrypted chaos` make
  no impersonation or invisibility claim, consistent with project positioning.

Expected benefit: readers can distinguish a reserved protocol enum value from
the active framing flow; reduced terminology ambiguity, with no runtime or
performance change expected (0%). Documentation-only review; runtime tests and
benchmarks do not apply. Validation: checked the cited definitions against
`crates/espejismo-core/src/protocol/framing.rs`,
`crates/espejismo-client/src/tunnel.rs`, and `docs/PROTOCOL.md`.
