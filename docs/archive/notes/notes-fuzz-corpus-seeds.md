# Fuzz corpus seeds for parsers and decoders

## Findings and approach

The repository already has cargo-fuzz targets for TOML configuration parsing, SOCKS5 UDP
packets, and native mux frames, but no checked-in seed corpus. The parsers branch on nested
TOML tables and semantic limits, SOCKS address types and length fields, and mux type/declared
payload length. Random mutation starts more effectively from structurally meaningful valid
inputs and near-valid boundary cases.

Following the reference list's emphasis on explicit framing boundaries and the project's
small-operations, authenticated-chaos positioning, this change adds only seed inputs and
run documentation. The corpus includes valid packets/configurations as well as truncated
headers, unknown variants, invalid semantic boundaries, and oversized declared lengths. It
does not change protocol behavior, camouflage, runtime code, or dependencies.

## Expected impact

The seeds should improve early mutation coverage of parser branches and length validation for
the three existing targets, while requiring no new tooling beyond cargo-fuzz. No numeric
coverage gain is claimed because cargo-fuzz/libFuzzer is not installed or run in this
workspace; corpus efficacy should be assessed with `cargo fuzz run <target>` and coverage
instrumentation when available. No runtime performance change is expected.

## Experiment

`$HOME/.cargo/bin/cargo test -p espejismo-core` passed: 120 unit tests, 1 integration test,
and 1 doctest, 0 failures. Fuzz execution and coverage measurement could not be performed
because `cargo fuzz --version` reports that the `fuzz` subcommand is not installed. The
change is seed-only, so runtime performance is unchanged by construction; no coverage
percentage is claimed. The measurable result for this patch is 24 curated seed files across
the three existing targets. No regressions were observed in the core test suite.
