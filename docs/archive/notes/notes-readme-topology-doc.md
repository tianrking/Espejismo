# README Deployment Topology

## Findings and change

- The README introduced the local proxy and remote server in separate sections,
  but did not show how traffic passes between them or where optional components
  fit.
- The runtime architecture confirms that SOCKS5/HTTP proxy flows and optional
  TUN flows enter `espejismo-local`, share authenticated multiplexed physical
  TCP tunnel lanes, and reach `espejismo-remote` for direct or configured
  upstream-proxy egress.
- Added a text topology diagram after the README quickstart, labeling client and
  server roles, local ingress options, tunnel lanes, supported TCP underlays,
  optional upstream proxy, and destination services.
- Added short explanatory text to clarify that SOCKS5 UDP relay uses the same
  TCP tunnel and that the topology does not claim camouflage or a UDP
  underlay.

## Rationale and expected result

The diagram gives readers a quick map of component relationships before they
encounter configuration details. Expected improvement: readers can identify
which machine runs each binary and how traffic moves without combining several
README paragraphs. This is a documentation usability improvement; no measured
performance change is expected.

## Review / validation

- Cross-checked component names, ingress types, TCP lane behavior, underlays,
  and egress against `docs/ARCHITECTURE.md`,
  `docs/deployment/CONFIG.md`, and `configs/examples/espejismo.toml`.
- Confirmed the wording remains consistent with `docs/POSITIONING.md`: one
  client, one remote server, no protocol camouflage, and no implication that
  physical UDP transport is the production path.
- Documentation-only change. No Cargo tests or throughput benchmarks apply;
  no behavioral or performance claims are made.
