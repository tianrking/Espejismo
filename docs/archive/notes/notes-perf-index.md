# Performance Documentation Index Notes

## Research and findings

Reviewed `docs/research/REFERENCES.md` and `docs/POSITIONING.md` first. The
references emphasize measuring representative paths and comparing repeated
runs; that method is useful here without adopting another project's transport
or product positioning. Existing material included a tuning guide, benchmark
methodology, and path-specific reports, but no page connected them.

## Change and expected benefit

- Added `docs/testing/PERFORMANCE_INDEX.md`, grouping tuning, measurement,
  recorded results, and research references.
- Linked the index from the README throughput reference and the deployment
  documentation index.
- Preserved the TCP/Yamux architecture, authenticated encryption, non-camouflage
  positioning, and small operational model.

Expected runtime performance change: **0%**. This is documentation only and
does not modify executable code, defaults, or protocol behavior. The expected
benefit is easier discovery of existing tuning advice and evidence; no
quantified reduction in search time is claimed.

## Validation and evidence

No before/after throughput experiment applies: the same binaries and runtime
settings execute before and after a documentation index change. No new runtime
performance gain is claimed. Existing measured context remains documented in
`docs/testing/THROUGHPUT_TUNING_HK2_RK.md`; for example, one five-round run
recorded four-way upload medians of 466.8 Mbit/s direct and 459.6 Mbit/s proxied
(101% same-window efficiency). These are path-specific observations, not a
measurement of this documentation change.

Checked the linked paths against the repository and reviewed the index
references against `docs/testing/BENCHMARKS.md` and
`docs/deployment/PERFORMANCE.md`. Cargo tests and a throughput benchmark are
not applicable because runtime behavior is unchanged. Documentation link
review passed; runtime regression risk is none from these edits.
