# CLI reference coverage

## Change and reason

Expanded `docs/deployment/CLI.md` with option-by-option references for
`espejismo-local`, `espejismo-remote`, and the optional
`espejismo-bench-http` helper. The guide now states that the programs use
options rather than subcommands, identifies config-controlled overrides, and
documents the benchmark helper's defaults and effective chunk-size range.

Previously the CLI guide showed common recipes but did not give operators one
place to look up every supported option. The reference follows the Clap
argument definitions in the three binaries. This is documentation only; no
runtime behavior or project positioning changes. Expected improvement is
complete discoverability of CLI arguments without changing runtime or
throughput performance.

## Verification

- Compared the documented option names with the Clap argument structs in all
  three binaries: client 53/53, remote 36/36, benchmark helper 4/4. No
  subcommand enum or positional argument is defined by these binaries.
- Checked the benchmark defaults and 1 KiB–1 MiB chunk clamp against
  `crates/espejismo-server/src/bin/bench_http.rs`.
- `git diff --check`: passed.
- No performance experiment or Rust test applies to this documentation-only
  change.
