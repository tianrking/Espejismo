# API rustdoc coverage

## Scope and rationale

- Added module-level rustdoc to the public `espejismo-core` areas that define
  configuration, authentication/crypto, protocol, transport, adapters, and
  operational interfaces. The descriptions state each module's role and
  important boundaries without promising behavior beyond the implementation.
- Fixed the bare Yamux specification URL so rustdoc renders it as a hyperlink.
- This is documentation-only: no signatures, runtime paths, protocol behavior,
  or dependencies changed. Expected performance change is 0%; the expected
  benefit is easier API discovery and clearer generated documentation.

## Validation

- Baseline `cargo doc --workspace --no-deps` completed but emitted one
  `rustdoc::bare_urls` warning for the Yamux specification link.
- After the changes, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace
  --no-deps` passed for all workspace packages. This confirms rustdoc emits no
  warnings under the warning-as-error gate.
- No runtime behavior changed, so a performance experiment is not applicable.
