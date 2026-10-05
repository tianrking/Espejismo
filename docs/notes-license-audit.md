# License audit

## Scope and method

Reviewed the root `LICENSE`, workspace and crate Cargo manifests, the vendored
`tokio-yamux` provenance note, and resolved Cargo license metadata on
2026-10-04. The dependency inventory came from `cargo metadata --offline
--format-version 1`; it describes the current lockfile and cached registry
metadata. This is a repository documentation audit, not a legal opinion or a
review of every source file's copyright history.

## Findings

- The root project and the three first-party crates declare MIT. The README
  links the root `LICENSE` and displays an MIT badge.
- The workspace includes `tokio-yamux 0.3.18` as a vendored crate. Its
  normalized and original Cargo manifests declare MIT; `VENDOR.md` documents
  its Nervos Network/Tentacle origin and local changes. The vendored directory
  contains no license text, and its source files contain no copyright or SPDX
  notice. The root MIT text names only “Espejismo contributors”; it does not
  carry an explicit upstream notice for this vendored code. Preserve the
  upstream-required notice when distributing the combined work. The exact
  notice text should be verified from an authoritative upstream source before
  adding it; it is not inferred here.
- The resolved graph contains 309 packages: 4 workspace packages and 305
  external packages. All 305 external packages provide Cargo `license` or
  `license_file` metadata. Expressions include permissive licenses, Unicode
  and CDLA-Permissive-2.0 terms. `r-efi 5.3.0` reports
  `MIT OR Apache-2.0 OR LGPL-2.1-or-later`; this is a choice of licenses in
  its metadata, not evidence that the application selected LGPL terms.
- Cargo metadata is not a substitute for reviewing license texts, source
  notices, feature-specific dependencies, or redistribution obligations. No
  automated license-policy tool was available in the environment, and this
  audit did not assert compatibility of every possible feature combination.

## Plan and expected benefit

Record the source and limits of this inventory so maintainers can distinguish
declared package metadata from a full compliance review. Before distributing
the vendored Yamux source, verify and include its authoritative MIT copyright
notice and license text in an appropriate third-party notice location. For
future release audits, consider a reproducible license-policy check that reads
the lockfile and documents allowed expressions and notice collection. These
documentation-only steps improve traceability and reduce the chance of
omitting an upstream attribution; they do not change runtime behavior or
produce a measurable performance gain.

## Validation and outcome

`cargo metadata --offline --format-version 1` succeeded. A local metadata
summary found 309 resolved packages, including 305 external packages, with no
external package missing both license fields. No tests were run because no
code or runtime behavior changed. The audit is complete as an inventory, with
the vendored MIT notice verification remaining an explicit release follow-up.
