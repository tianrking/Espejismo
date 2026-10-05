# Deployment index notes

## Scope and rationale

- Reviewed the existing `docs/deployment/INDEX.md` and confirmed it already
  catalogs the deployment and operations guides and is linked from both README
  language editions.
- Made its opening section task-oriented so new deployments, production service
  setup, upgrades/recovery, and incident diagnosis each have a clear next step.
- Kept the index as links to existing guides; no new deployment mechanism,
  runtime behavior, or product positioning was introduced.

## Expected improvement

This is a documentation-only navigation improvement. Runtime performance
change is 0%. The scenario paths should reduce the number of choices an
operator must scan before reaching the relevant guide. No reduction in support
time is claimed because no operator study was run.

## Validation

- Confirmed the index is linked in `README.md` and `README_ES.md`.
- Checked all 52 Markdown links in `docs/deployment/INDEX.md`; all targets
  resolve to files in the repository.
- Documentation-only change; Cargo tests and throughput benchmarks do not
  apply.
- Outcome: link validation passed; runtime regressions are not applicable.
