# Roadmap

This page summarizes the project's current areas of work for users and
contributors. It is a directional overview, not a release schedule or a promise
that every item will ship. Priorities can change as testing and maintenance
needs evolve. The project remains a small, native Rust encrypted tunnel; see
[project positioning](POSITIONING.md).

## Current focus

- **Reliability on long-haul links.** Investigate and resolve the intermittent
  long-transfer failure recorded in [known issues](KNOWN_ISSUES.md), and keep
  publishing reproducible throughput measurements in
  [performance documentation](testing/PERFORMANCE_INDEX.md).
- **Stability and operational clarity.** Continue improving bounded resource
  use, recovery behavior, diagnostics, and deployment documentation while
  keeping the two-binary, one-config operating model.

## Possible follow-on work

The following items are implementation gaps or exploratory directions, not
scheduled features:

- **Active stream migration.** Explore whether existing logical streams can
  survive replacement of a physical tunnel. The current implementation only
  reconnects lanes for new streams.
- **UDP underlay integration.** The repository has UDP packet and reliability
  primitives, but no integrated UDP underlay. Any future work needs to preserve
  the documented distinction between SOCKS5 UDP relay and transport underlay.
- **Platform and tooling gaps.** Consider OS-specific TCP congestion telemetry,
  richer profile controls, and log rotation where these add practical value
  without expanding the operating model unnecessarily.
- **Separate browser/WASM direction.** Browser support would require a separate
  transport-oriented crate reusing suitable protocol primitives; the current
  Tokio TCP binaries are native-first. Browser extension packaging is also not
  implemented.

## Scope boundaries

The roadmap does not include protocol camouflage, a broad multi-protocol
platform, or a centralized account service. Espejismo continues to use
authenticated encrypted traffic without pretending to be TLS, HTTP, or QUIC;
this is not a claim of invisibility. See [positioning](POSITIONING.md) and the
[protocol specification](PROTOCOL.md) for the product and wire-level contracts.

For implemented capabilities and the current gap list, see
[implementation status](development/STATUS.md). Operationally relevant
limitations are tracked in [known issues](KNOWN_ISSUES.md).
