# Tuning Profile Use Cases

## Findings

Reviewed `docs/research/REFERENCES.md` and `docs/POSITIONING.md` before editing.
The references favor representative, repeated measurement; this task only
clarifies documentation and does not borrow another project's transport or
product positioning. The existing profile descriptions in
`docs/deployment/PROFILES.md` named the overlays but did not clearly distinguish
their workloads, costs, or the remote-policy purpose of `server-safe`.
`docs/deployment/PERFORMANCE.md` provided a partial selection table.

Checked the named overlay implementation in
`crates/espejismo-core/src/config/mod.rs::apply_named_profile` to keep the
scenarios tied to actual settings. In particular, `fast` uses an 8 MiB tunnel
buffer and four bulk lanes, while `auto-throughput` uses at least 16 MiB buffers
and six bulk lanes; `server-safe` affects remote egress/resource policy rather
than being a client throughput profile. Named overlays remain distinct from
`shared.obfuscation.profile` values.

## Change And Rationale

- Expanded `docs/deployment/PROFILES.md` with a workload/policy scenario,
  trade-off, and deployment side for every named profile.
- Added `fast` and remote `server-safe` to the selection table in
  `docs/deployment/PERFORMANCE.md`, and linked to the detailed profile guide.
- Clarified that stealth shaping is not protocol camouflage, in line with
  `docs/POSITIONING.md`.

Expected runtime performance change: **0%**. No executable code, configuration
defaults, or protocol behavior changed. Expected user benefit is easier profile
selection and fewer accidental policy or memory trade-offs.

## Validation And Evidence

This is a pure documentation change. A before/after throughput benchmark would
run identical binaries and cannot measure the documentation benefit, so no new
benchmark was run and no runtime gain is claimed. Existing path-specific
measurements remain documented in
`docs/testing/THROUGHPUT_TUNING_HK2_RK.md` and
`docs/deployment/PERFORMANCE.md`; they are not presented as profile guarantees.
Reviewed the descriptions against `apply_named_profile` and checked the edited
Markdown for consistency with the current profile names and documented
positioning. Runtime regression risk: none from these documentation-only edits.
