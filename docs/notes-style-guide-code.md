# Rust Code Style Guide Notes

## Research and findings

The repository already asks contributors to use `cargo fmt`, keep Clippy
warnings clear, prefer small functions and explicit errors, and avoid unsafe
Rust (`CONTRIBUTING.md`). Existing core modules provide public rustdoc and
reusable APIs; protocol and configuration behavior are documented separately
in `docs/PROTOCOL.md` and the configuration guides. There was no single
contributor reference covering module ownership, async work, error handling,
and security-sensitive code conventions.

The new `docs/RUST_CODE_STYLE.md` consolidates those existing expectations and
adds practical guidance grounded in workspace patterns: narrow visibility,
bounded network input, cancellation-aware async I/O, avoiding locks across
`.await`, and testing malformed input and boundary conditions. It does not
change runtime code, protocol behavior, or Espejismo's positioning.

## Plan and expected effect

- Add one canonical Rust style page and link it from `CONTRIBUTING.md`.
- Keep requirements actionable and defer validation commands to the existing
  contributor guide instead of duplicating the full workflow.
- Expected benefit is lower contributor and review ambiguity. This is a
  documentation-only change; no runtime performance gain is expected or
  claimed, and no quantitative quality improvement was measured.

## Validation and experiment

- Manually compared the guide with `CONTRIBUTING.md`, `docs/PROTOCOL.md`,
  `docs/POSITIONING.md`, and representative public API and async protocol code.
- Reviewed relative links and terminology against the repository paths and
  canonical references.
- No code changed, so compilation and Rust tests were not run. This change
  makes no correctness or performance claim. No regression is expected because
  only contributor documentation changed.
- `git diff --check`: passed with no whitespace errors.
