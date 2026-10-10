# Contributor Onboarding

This guide is a short path from a fresh checkout to a focused, reviewable
contribution. Use the linked guides for the canonical details rather than
copying their full procedures here.

## 1. Learn the project boundaries

Read [project positioning](POSITIONING.md) first. Espejismo is a native
encrypted tunnel with a small operating model; its core protocol does not
pretend to be TLS, HTTP, or QUIC. Optional WebSocket and HTTP/2 underlays use
those real transports. Keep proposals within these boundaries.

Then skim [the architecture guide](ARCHITECTURE.md) for component ownership
and [the code tour](CODE-TOUR.md) for source paths. The workspace has four
crates:

- `espejismo-core`: shared configuration, protocol, crypto, and transport.
- `espejismo-client`: the `espejismo-local` client binary.
- `espejismo-server`: the `espejismo-remote` server binary.
- `tokio-yamux`: the workspace's yamux implementation.

The separate `fuzz/` package is not a workspace member. `configs/` contains
maintained configuration examples, and `scripts/` contains developer and
packaging helpers.

## 2. Find the right source of truth

Start from the behavior you plan to change:

- CLI, client startup, or proxy ingress: `crates/espejismo-client/src/`.
- Server startup, peer handling, or relay: `crates/espejismo-server/src/`.
- Shared handshake, frames, config, or transport: `crates/espejismo-core/src/`.
- Multiplexing behavior: the owning crate's mux module or `crates/tokio-yamux/`.

For wire behavior, read [the protocol specification](PROTOCOL.md) alongside
the implementation. For configuration and operations, use the relevant page
in [the deployment index](deployment/INDEX.md). Check existing tests near the
code and search for callers before changing a shared API.

## 3. Set up and make a focused change

Install stable Rust and Cargo, then work from the repository root. Build and
validate with the commands in [Contributing](../CONTRIBUTING.md) and choose
the smallest useful iteration checks from the [testing guide](testing/TESTING_GUIDE.md).
Keep a change focused, add regression coverage for behavior changes, and update
the canonical docs when supported behavior or configuration changes.

Before proposing a design that changes architecture, protocol compatibility,
security boundaries, or operational complexity, follow the decision process in
the contributing guide. Do not claim invisibility or camouflage; consult
[security guidance](SECURITY.md) for trust boundaries.

## 4. Prepare the review

Review the diff for scope, secrets, generated files, and stale documentation.
Report only checks that you actually ran. Documentation changes should have
their relative links and copyable commands checked; Rust changes should include
the relevant tests and quality checks. Performance claims need comparable
measurements using the [benchmark guide](testing/BENCHMARKS.md).

Use the [commit guide](COMMIT_GUIDE.md) for concise English commit subjects.
The [code review checklist](development/CODE_REVIEW_CHECKLIST.md) explains
what authors and reviewers should verify before merge.
