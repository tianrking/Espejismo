# Performance Documentation Audit

## Scope and findings

Audited `docs/deployment/PERFORMANCE.md` against the config defaults and named
profile overlays in `crates/espejismo-core/src/config/{defaults.rs,mod.rs}`, the
client RTT controller in `crates/espejismo-client/src/adaptive.rs`, and the
Yamux adapter/config in `crates/espejismo-core/src/transport` and
`crates/tokio-yamux/src/config.rs`.

- The documented `auto-throughput` values match the implementation: 16 MiB
  tunnel buffer and minimum mux window, at least 4 MiB per TCP socket buffer,
  64 KiB to the 262127-byte normal-frame payload cap, one interactive plus six
  bulk lanes (pool maximum 8), and at least 512 streams.
- The default maximum Yamux stream window is 8 MiB, while the initial stream
  window remains 256 KiB. Espejismo passes the configured maximum to Yamux;
  shared max streams defaults to 256. Bundled Yamux keepalive is enabled at 30
  seconds and the adapter inherits it. The current performance document
  describes these accurately.
- Clarified runtime adaptive sizing to match the controller: the mux window
  floor follows every smoothed RTT sample for default-valued fields, while
  tunnel and TCP buffer boosts engage after two samples above 150 ms and
  release after three samples below 80 ms. Changes affect new sessions. The
  BDP estimate assumes 1 Gbit/s and is capped; it is not path-bandwidth
  discovery. Profile-set or explicitly configured fields are not eligible.

## Change and rationale

Updated the adaptive-throughput paragraph in `docs/deployment/PERFORMANCE.md`
to state the exact sampling, hysteresis, and session behavior. This avoids
implying that adaptive sizing is a user-enabled profile toggle or that it
changes every existing connection. The guidance preserves the project's TCP,
Yamux, authenticated-encryption, and no-camouflage positioning.

## References and validation

Read `docs/research/REFERENCES.md` and `docs/POSITIONING.md` as required. Their
measurement-first advice supports documenting observed implementation
behavior rather than promising universal throughput gains; no upstream
mechanism or protocol was introduced.

This is a documentation-only audit. No runtime or performance behavior
changed, so before/after throughput numbers would not measure this edit. The
documented values were checked directly against implementation constants and
profile logic; no compile or test run was needed.
