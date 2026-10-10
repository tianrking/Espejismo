# Rust Code Style

This guide records the conventions for Rust changes in Espejismo. It is for
contributors working in the workspace crates; follow it alongside the
[contributing guide](../CONTRIBUTING.md), [architecture guide](ARCHITECTURE.md),
and [project positioning](POSITIONING.md).

## Formatting and structure

- Use stable Rust and let `cargo fmt --all` format the change. Avoid unrelated
  formatting churn.
- Keep modules focused on one responsibility and place shared protocol,
  configuration, and runtime behavior in `espejismo-core` where appropriate.
  Keep client and server orchestration in their respective crates.
- Prefer small functions with names that describe the operation. Make state
  transitions and resource ownership easy to follow; avoid clever control flow
  and unnecessary abstraction.
- Use descriptive names and standard Rust naming: `snake_case` for modules,
  functions, and variables; `UpperCamelCase` for types and traits; and
  `SCREAMING_SNAKE_CASE` for constants.
- Group imports by crate or module and keep the list limited to names used in
  that module. Follow rustfmt's layout rather than hand-aligning imports.

## APIs and documentation

- Keep visibility as narrow as practical. Add rustdoc to public types and
  functions to explain purpose, important constraints, and non-obvious behavior.
- Include examples for public APIs when they clarify correct use. Ensure code
  examples reflect supported configuration and protocol behavior.
- Treat serialized configuration and wire formats as compatibility contracts.
  Before changing them, consult the canonical configuration documentation and
  [protocol specification](PROTOCOL.md).
- Comment on why a non-obvious decision is needed, especially around security,
  concurrency, resource bounds, and protocol parsing. Avoid comments that only
  paraphrase the next line of code.

## Errors and async work

- Return errors to the caller instead of silently discarding failures. Use `?`
  for propagation and add context at a boundary where it helps identify the
  failed operation.
- Do not use `unwrap` or `expect` on values influenced by configuration,
  network input, or runtime state. In tests, they are appropriate when failure
  should fail the test. In production code, use them only for a documented
  invariant that cannot fail through normal operation.
- Keep async I/O bounded and cancellation-aware. Avoid holding a synchronous
  lock across `.await`; bound queues, buffers, and peer-controlled lengths.
- Avoid detached work without a clear owner and shutdown path. Follow existing
  Tokio task and channel patterns in the owning subsystem.

## Security and tests

- Do not add `unsafe`; the workspace denies it. Keep authentication,
  encryption, replay defense, and resource-limit checks explicit and reviewable.
- Treat all network data as untrusted. Validate lengths and structure before
  allocation or use, and avoid logging credentials, keys, or private payloads.
- Credential-bearing types should redact secrets in `Debug` output so routine
  diagnostic formatting cannot expose proxy usernames, passwords, or URIs.
- Add focused regression tests for behavior changes. Keep unit tests near the
  implementation when practical and integration tests under the crate's
  `tests/` directory. Test malformed input and boundary conditions for parsers
  and protocol code.
- Run the relevant checks listed in [Contributing](../CONTRIBUTING.md), and
  report only checks that were actually run. Performance or security claims
  require evidence appropriate to the claim.

## Review checklist

- The change follows existing module ownership and keeps public surface area
  intentional.
- Errors, cancellation, input bounds, and resource cleanup have been
  considered.
- Tests cover behavior changes and relevant boundary cases.
- Formatting and documentation match the repository conventions.
- Protocol and product claims remain consistent with the canonical references.
