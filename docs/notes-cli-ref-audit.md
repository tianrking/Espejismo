# CLI reference audit

## Audit and result

Compared the option tables in `docs/deployment/CLI.md` with the rendered
`--help` output from the current workspace builds of `espejismo-local`,
`espejismo-remote`, and `espejismo-bench-http`. All documented option names
match: 53/53 client options, 36/36 remote options, and 4/4 benchmark-helper
options. No documented option is stale, and no supported option is missing.

Reviewed the displayed descriptions and benchmark helper defaults against the
reference. The 1 KiB–1 MiB effective `--chunk-bytes` range and the documented
defaults agree with the rendered help. The reference also explains that the
service binaries have no subcommands and points readers to process exit-code
and configuration references. No CLI reference correction was warranted, so
the user-facing CLI documentation remains unchanged.

This audit adds only its findings and reproducible verification method. No
runtime, project-positioning, performance, or behavior change is expected.

## Verification

- Built both packages and all binaries with
  `$HOME/.cargo/bin/cargo build --offline -p espejismo-client -p espejismo-server --bins`.
- Captured `target/debug/espejismo-local --help`,
  `target/debug/espejismo-remote --help`, and
  `target/debug/espejismo-bench-http --help`; compared each parsed option set
  with its corresponding table in `docs/deployment/CLI.md`.
- Result: 93 documented options matched 93 rendered options, with zero missing
  or stale entries. The benchmark helper's help shows defaults of `0.0.0.0:18082`,
  256 MiB, 4096 MiB, and 262144 bytes, plus the 1 KiB–1 MiB clamp; the reference
  agrees.
- No Rust tests or performance experiment apply because no runtime code or
  user-facing documentation needed changes.
