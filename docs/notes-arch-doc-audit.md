# Architecture Documentation Audit

## Scope and findings

Compared `docs/ARCHITECTURE.md` with the workspace crate manifests and Rust
module declarations. The data-path, handshake, mux, configuration, and egress
sections describe runtime behavior; this pass focused on the source-layout map.

- The source-layout section listed only a subset of `espejismo-core`, leaving
  out existing modules for admin, CLI support, DNS, egress, extension hooks,
  logging, metrics, profiles, runtime state, TCP helpers, underlay adapters,
  and update checks.
- It did not map either executable's modules, platform routing/TUN code, the
  standalone HTTP benchmark binary, or the workspace `tokio-yamux` crate.
- Its short module descriptions were broadly consistent, but calling UDP
  reliability primitives simply “UDP underlay primitives” risked conflating
  code in `protocol/udp.rs` with the production transport path. The existing
  production-vs-experimental clarification elsewhere in the document remains
  the source of truth.

## Change and rationale

Expanded `docs/ARCHITECTURE.md`'s source-layout section to reflect the current
workspace: shared core responsibilities, client and server executable module
groups, the independent benchmark executable, and the bundled Yamux crate.
Descriptions point to actual source directories and state that the map is not
a stable module API. This makes architecture navigation more complete without
changing runtime behavior or Espejismo's no-camouflage, small-operations-model
positioning.

Expected benefit: maintainers can locate the relevant implementation more
quickly and are less likely to mistake the native mux or experimental UDP
primitives for the production Yamux/TCP path. There is no runtime or performance
gain to quantify for this documentation-only change.

## Validation

Checked the top-level workspace crates and the module declarations in
`crates/espejismo-core/src/lib.rs`, client/server `main.rs`, and the Yamux
manifest/source tree. The listed component ownership matches those sources.
No code or configuration changed, so compilation, tests, and throughput
benchmarks do not validate this documentation edit and were not run.
