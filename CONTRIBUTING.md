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
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo check --manifest-path fuzz/Cargo.toml
```

For Rust changes, run the checks that cover the affected code before opening a
change. Formatting, workspace check, Clippy, and workspace tests match the CI
Rust job on Linux, macOS, and Windows. The fuzz manifest check runs in CI on
Linux. `cargo build --workspace` is a useful local build check; CI's separate
release-binary job builds its listed targets on Linux, macOS, and Windows. Call
out platform-specific limits you could not verify. See
[`docs/testing/TEST_PLAN.md`](docs/testing/TEST_PLAN.md) for test scope and
manual smoke checks.

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

- Follow the [Rust code style guide](docs/RUST_CODE_STYLE.md), `cargo fmt`
  formatting, and Clippy without warnings.
- Prefer small, readable functions and explicit error handling. Unsafe Rust is
  forbidden by the workspace lint.
- Preserve the project's security boundaries: authenticated encryption,
  bounded resource use, and quiet rejection of unauthenticated probes. Do not
  introduce claims that Espejismo is invisible or provides protocol camouflage.
- Write documentation in plain language, keep examples aligned with the current
  configuration and CLI, and link to the canonical guide instead of duplicating
  long instructions. Follow the shared [documentation style guide](docs/DOCUMENTATION_STYLE.md)
  for page structure, terminology, formatting, and review.
- For translated pages, follow the [translation contribution guide](docs/TRANSLATION_GUIDE.md)
  to keep technical meaning and source links aligned.
- Keep changelog entries user-facing. Follow
  [`docs/development/CHANGELOG.md`](docs/development/CHANGELOG.md) when a change
  has a notable user, compatibility, security, or operational effect.

## Submitting a change

Use the concise imperative subject and optional explanatory body described in
the [commit message guide](docs/COMMIT_GUIDE.md). Keep commit messages focused
on the change and its motivation.

Open a pull request with a concise title and a description that explains the
problem, the user-visible or operational effect, and the approach taken. Keep
each PR focused; split unrelated work so reviewers can assess each change on
its own. Link related issues when available, and call out compatibility,
configuration, security, or deployment effects explicitly.

Include a validation section in the PR description. List the checks you ran
and their results, mention platform coverage and any limits you could not
verify, and attach comparable benchmark conditions and results for performance
work. For documentation-only changes, identify the pages or procedures
reviewed, check changed relative links and copyable commands against their
canonical sources, and run `git diff --check`. Do not report a check as passing
if it was not run; state why a relevant check was skipped.

### Review expectations

The repository's [CODEOWNERS file](.github/CODEOWNERS) names the default
maintainer for review requests across the project. Update it when repository
ownership changes; keep entries tied to GitHub usernames or teams that can
actually review changes. Code ownership routes review requests but does not
replace the review expectations below or the required CI checks.

Reviewers should check that a change solves the stated problem without
expanding its scope, follows the architecture and product boundaries, and
handles relevant failure cases. For code, review tests, resource limits,
authentication and encryption boundaries, and compatibility effects. For
documentation, check technical accuracy against the implementation and
canonical references, working links, copyable examples, and clear limits on
claims. Performance claims need reproducible evidence; security-sensitive
changes need particular scrutiny of trust boundaries and failure behavior.

Keep review feedback specific and actionable, and distinguish required fixes
from suggestions. Authors should respond to each requested change, update the
PR description when its scope or validation changes, and rerun affected checks
after revisions. Resolve a discussion once its concern is addressed or the
participants agree on the outcome. Before merge, confirm the final diff is
focused, required checks are green, and the PR description still matches the
resulting change.

### Decision process

For a focused change within established behavior and architecture, explain the
problem, proposed change, and validation in the pull request. Record decisions
that affect architecture, protocol compatibility, security boundaries,
supported behavior, or operational complexity in the pull request discussion
before implementation. Include the context, constraints, alternatives
considered, expected effects, and evidence or checks that would validate the
choice. Link relevant code, documentation, benchmarks, or issue reports.

Check proposals against [project positioning](docs/POSITIONING.md) and the
[protocol specification](docs/PROTOCOL.md) where relevant. If a proposal would
conflict with either, stop implementation and discuss the product or protocol
direction first. After a decision, update the canonical documentation and
tests affected by it; keep the pull request description and validation results
consistent with the final change. For ongoing or cross-release decisions,
record the outcome in the appropriate development or architecture document so
future contributors can find the rationale without relying on old discussion.

Use short, focused commits with concise imperative English subject lines.
Avoid bundling generated output, local build artifacts, secrets, or unrelated
formatting changes.

For security-sensitive issues, avoid posting exploit details publicly before a
fix is available. Contact the maintainers through a private channel.

For bug reports, documentation corrections, and feature requests, use the
[support guide](docs/SUPPORT.md), which includes the public issue link and a
sanitized report template. Suspected vulnerabilities must follow the private
[security policy](SECURITY.md).
