# Client/server compatibility matrix

## Findings and plan

- The workspace release version is `0.1.5`; tags exist from `v0.1.0` through
  `v0.1.5`.
- `docs/PROTOCOL.md` defines wire protocol version `1` and says its contract
  covers the `v0.1.x` line through `v0.1.5`. The current handshake requires an
  exact protocol-version match and has no fallback.
- The maintained references do not provide a per-release wire-version history
  or test results for every client/server release pair. Equal binary versions
  therefore cannot be represented as a blanket cross-version guarantee.
- `docs/deployment/VERSION-COMPATIBILITY.md` had upgrade policy and a checklist,
  but no release-pair matrix.

Add an evidence-based matrix to that deployment guide. It identifies the
documented `v0.1.5` same-release baseline and explicitly marks historical and
future combinations unverified absent release-specific evidence. This should
help operators distinguish known protocol facts from assumptions and choose a
coordinated upgrade when pairwise compatibility is unknown. It preserves the
strict protocol-version handshake, small operational model, and no-camouflage
positioning.

Expected improvement: compatibility guidance is easier to apply across the six
tagged releases. Runtime, protocol, and performance improvement: none; this is
documentation only.

## Validation

- Cross-checked the workspace version, tags, protocol specification,
  compatibility guide, versioning guide, and project positioning.
- Confirmed the changed matrix makes no unsupported claim about historical
  release pairs and links its evidence boundary to release notes.
- No Rust code or behavior changed; compilation, tests, and throughput
  benchmarks are not applicable.

## Conclusion

The compatibility guide now includes an evidence-based release matrix and
states how operators should handle unverified pairs. No behavior or measurable
performance change is claimed.
