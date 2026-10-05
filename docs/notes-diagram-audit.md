# Architecture Diagram Audit

## Scope and findings

Reviewed the textual deployment topology in `README.md`, the architecture map
in `docs/ARCHITECTURE.md`, and its rendered source `docs/architecture.svg`.
Cross-checked the diagram labels against the client/server underlay selection,
mux configuration, and SOCKS5 UDP relay implementation.

- The SVG labeled the mux as `yamux mux`, which implied Yamux was the only
  available mode. The implementation also supports the in-tree native mux
  beta, while Yamux remains the production default.
- The SVG did not mention WebSocket and HTTP/2 underlays. Both are optional
  adapters over TCP; raw TCP remains the default physical underlay.
- SOCKS5 UDP relay is carried over the same TCP tunnel, not a UDP underlay.
  The SVG did not show this distinction, although the architecture prose and
  README already explained it correctly.
- The README topology and surrounding explanation match the current path:
  SOCKS5/HTTP or optional TUN ingress, authenticated encrypted TCP lanes,
  optional WebSocket/HTTP/2 underlay, and remote direct or upstream-proxy
  egress. No contradictory text diagram was found.

## Change and expected benefit

Updated the SVG mux label to show both configured modes and added a compact
caption that names the optional TCP underlays and locates SOCKS5 UDP relay
inside the TCP tunnel. This removes two likely topology misunderstandings and
aligns the rendered architecture graphic with its adjacent prose and current
implementation. Expected runtime/performance change: none; the benefit is more
accurate architecture documentation.

## Validation

Checked the underlay adapters and server dispatch in
`crates/espejismo-core/src/underlay.rs` and
`crates/espejismo-server/src/handler.rs`, mux configuration and stream handling
in the client/server modules, and SOCKS5 UDP relay paths. Parsed the edited SVG
as XML and reviewed the final diff. This is a documentation-only change, so
Rust compilation, tests, and throughput benchmarks are not applicable and
were not run.
