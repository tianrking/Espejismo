# Architecture Diagram Supplement

## Findings and change

- `docs/ARCHITECTURE.md` already described the protocol, mux, lifecycle, and
  configuration in detail and linked to a visual topology SVG. It lacked a
  compact text-only map showing how local ingress flows through the client,
  physical lanes, remote session handling, and egress.
- Added a monospaced end-to-end architecture map near the start of the document,
  followed by brief notes on configuration ownership, stream conversion,
  application-level UDP relay, and the boundary between the TCP production path
  and experimental UDP underlay primitives.
- Cross-checked the map against the client tunnel/handler structure, server
  handler/relay structure, existing architecture sections, and
  `docs/POSITIONING.md`.

## Rationale and expected result

Readers can trace the main data path without first combining details from later
sections or relying on SVG rendering. Expected benefit: faster orientation and
fewer mistaken assumptions about which layer handles ingress, authentication,
multiplexing, and egress. This is documentation usability only; expected
performance improvement is 0% and no runtime behavior changes.

## Review / validation

- Documentation-only scope; compilation, tests, and throughput benchmarks do
  not apply because no executable code or performance behavior changed.
- Reviewed the Markdown additions and diff for consistency with the existing
  architecture description and product positioning. Result: the map identifies
  TCP as the ordinary physical path, leaves WebSocket/HTTP/2 as optional TCP
  underlays, and does not imply protocol camouflage or production UDP
  transport.
