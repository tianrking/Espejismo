# Capacity planning method

## Findings and approach

Reviewed `docs/POSITIONING.md` and `docs/research/REFERENCES.md` first, then
checked `docs/deployment/RESOURCES.md`, `PERFORMANCE.md`, configuration defaults,
profile overlays, and adaptive buffer sizing. Existing resource documentation
listed the principal limits but left the process of turning limits into a host
budget underspecified. In particular, configured buffer capacity, observed RSS,
and native mux flow-control credit must remain distinct quantities.

Expanded `docs/deployment/RESOURCES.md` with a per-process estimation workflow:
select an expected workload, calculate the active-lane buffer budget using the
effective post-profile buffer, measure incremental RSS under representative
stream/lane counts, account separately for variable queues and descriptors,
and validate with peak metrics plus headroom. Added a default client example
explicitly labeled as configured capacity rather than RSS. No runtime setting
or behavior changed; this keeps the TCP/Yamux design, no-camouflage positioning,
and small-operations model intact.

## Expected impact and validation

This is documentation only. Expected throughput, CPU, and runtime memory change
is 0%; the benefit is clearer and less error-prone operator estimates. A
throughput comparison and cargo tests do not apply because no executable code
changed. The values and terminology were checked against current config and
runtime code. `git diff --check` is the applicable repository hygiene check;
there is no performance result to report for a documentation-only change.
