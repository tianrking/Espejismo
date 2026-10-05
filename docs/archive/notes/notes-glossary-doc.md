# Glossary documentation

## Analysis and plan

- Reviewed `docs/POSITIONING.md`, `README.md`, `docs/PROTOCOL.md`,
  `docs/ARCHITECTURE.md`, and deployment configuration/profile documentation.
- The same core vocabulary recurs across onboarding and implementation docs:
  lane, physical connection, logical stream, mux, underlay, frame, handshake
  window, profile, TUN, and UDP relay. These terms describe different layers
  and can be conflated, especially UDP relay versus the experimental UDP
  underlay primitives.
- Add a root `GLOSSARY.md` defining terms in the project's own scope. Explicitly
  preserve the positioning that Espejismo does not impersonate protocols and
  makes no invisibility guarantee. Distinguish production TCP, SOCKS5 UDP
  relay over TCP, and experimental UDP underlay primitives.
- This is documentation-only; expected runtime performance change is 0%. The
  expected benefit is lower terminology ambiguity for readers moving between
  setup, architecture, and protocol documentation.

## Validation and outcome

- Cross-checked definitions against the current protocol and architecture docs
  and the positioning statement. No code, protocol behavior, dependencies, or
  configuration defaults changed.
- Correctness is validated by consistency review against those source docs;
  runtime tests and performance benchmarks do not apply to this documentation
  change.
- Outcome: glossary coverage added with the distinctions above; no runtime
  regression is expected because executable files are unchanged.
