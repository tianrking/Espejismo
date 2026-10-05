# Brand usage

This guide defines the project name and logo usage in repository materials.
Use it for documentation, release notes, diagrams, and project-facing copy.

## Project name

- Write the product and project name as **Espejismo**, preserving this exact
  capitalization and spelling.
- Use `espejismo` in lowercase for package names, executable names, paths,
  configuration keys, environment variables, and other literal identifiers.
- Use `espejismo-local` for the client and `espejismo-remote` for the server.
  Do not treat these component names as alternate spellings of the project.
- In prose, use “Espejismo” on first reference and when the project name is
  needed later. Avoid “ESPEJISMO”, “EspeJISMO”, and informal abbreviations.
- Preserve upstream names, URLs, repository identifiers, and historical text
  when quoting or referring to them; do not rewrite literal technical values
  just to match prose casing.

## Logo and wordmark

The repository currently defines no approved graphical logo or logo asset.
Use the plain-text wordmark **Espejismo** in its standard capitalization.
Do not create or imply an official icon, mascot, alternate lockup, or registered
mark. If a graphical logo is introduced later, add its source asset and approved
variants here before using it across project materials.

Keep the wordmark legible and unmodified. Do not stretch it, recolor individual
letters, add effects, or combine it with partner marks in a way that suggests
endorsement. In diagrams, use `Espejismo` for the product and lowercase
component identifiers when referring to binaries or roles.

## Product claims

Brand copy must follow [the project positioning](POSITIONING.md): describe
Espejismo as an encrypted tunnel whose core protocol does not impersonate other
protocols. Do not use logo treatments or copy that promise invisibility,
undetectability, or traffic camouflage. Optional WebSocket and HTTP/2 underlays
are real transports and must not be presented as disguises.

## Applying this guide

When updating a page, preserve literal technical identifiers and correct
inconsistent prose references where practical. For product boundaries and
technical terminology, defer to [project positioning](POSITIONING.md) and the
[documentation style guide](DOCUMENTATION_STYLE.md).
