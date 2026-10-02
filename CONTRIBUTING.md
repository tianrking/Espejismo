# Contributing to Espejismo

Thanks for considering a contribution. Espejismo is a native Rust encrypted
tunnel built around a deliberately small operating model: one client, one
server, and one TOML configuration. It does not impersonate TLS, HTTP, QUIC, or
other protocols. Contributions should preserve that direction; see
[`docs/POSITIONING.md`](docs/POSITIONING.md) before proposing changes to product
behavior or protocol design.

## Development setup

Install a stable Rust toolchain with Cargo, then clone the repository. The
workspace uses the root `Cargo.toml` and contains the core, client, server, and
`tokio-yamux` crates. Fuzzing is maintained separately in `fuzz/`.

Build the workspace and run the required checks from the repository root:

```bash
cargo build --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo check --manifest-path fuzz/Cargo.toml
```

If you change Rust code, run the checks that cover the affected code before
opening a change. The commands above are the repository's full quality gate;
cross-platform behavior also matters, so call out any platform-specific limits
that you could not verify. See [`docs/testing/TEST_PLAN.md`](docs/testing/TEST_PLAN.md)
for test scope and manual smoke checks.

## Making a change

1. Read the relevant crate code and documentation first. Check
   [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for component boundaries and
   [`docs/PROTOCOL.md`](docs/PROTOCOL.md) before touching wire behavior.
2. Keep changes focused and consistent with existing patterns. Add or update
   regression tests for behavior changes, especially protocol, authentication,
   configuration, and resource-limit behavior.
3. Update user-facing documentation when commands, configuration, supported
   behavior, deployment, or compatibility changes. Put operational guidance in
   `docs/deployment/`; keep protocol contracts in `docs/PROTOCOL.md`.
4. Do not claim performance, reliability, or security improvements without
   evidence. For throughput changes, record comparable before-and-after results
   using the benchmark process in [`docs/testing/BENCHMARKS.md`](docs/testing/BENCHMARKS.md).
5. Review the final diff for accidental generated files, secrets, unrelated
   formatting, and stale documentation.

## Rust and documentation conventions

- Follow `cargo fmt` formatting and keep Clippy warnings clear.
- Prefer small, readable functions and explicit error handling. Unsafe Rust is
  forbidden by the workspace lint.
- Preserve the project's security boundaries: authenticated encryption,
  bounded resource use, and quiet rejection of unauthenticated probes. Do not
  introduce claims that Espejismo is invisible or provides protocol camouflage.
- Write documentation in plain language, keep examples aligned with the current
  configuration and CLI, and link to the canonical guide instead of duplicating
  long instructions.
- Keep changelog entries user-facing. Follow
  [`docs/development/CHANGELOG.md`](docs/development/CHANGELOG.md) when a change
  has a notable user, compatibility, security, or operational effect.

## Submitting a change

Provide a concise summary of the behavior and motivation, list the checks you
ran, and include relevant benchmark results for performance work. Mention
platform coverage and any known limitations. Keep commits focused and use a
short imperative English subject line.

For security-sensitive issues, avoid posting exploit details publicly before a
fix is available. Contact the maintainers through a private channel.
