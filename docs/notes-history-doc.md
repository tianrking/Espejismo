# History documentation notes

## Plan and rationale

Add a concise, reader-facing development history at
`docs/development/HISTORY.md`. The existing root changelog already records
release details, while implementation status lists capabilities; neither gives
a short narrative of the project's stages. The new page groups releases by
their main direction and links back to canonical status, roadmap, positioning,
and changelog pages. Add it to the development documentation index.

The history is grounded in the release headings and summaries in
`CHANGELOG.md`, plus the architecture and positioning documents. It describes
themes rather than making a comprehensive feature inventory. It preserves the
documented distinction between SOCKS5 UDP relay, experimental UDP primitives,
and the production TCP underlay, and does not imply camouflage or invisibility.

## Expected benefit

Readers can understand the project's progression without scanning every
changelog entry. This is a documentation-only change: it has no runtime or
performance effect, so a throughput benchmark is not applicable. No code
correctness behavior changed; validation consists of checking the history's
claims and links against repository documents.

## Validation

- Compared release milestones and feature descriptions with the root changelog,
  `docs/ARCHITECTURE.md`, `docs/POSITIONING.md`, `docs/development/STATUS.md`,
  and `docs/ROADMAP.md`.
- Verified all newly added relative links resolve to tracked documentation.
- No performance claim is introduced; no runtime tests are applicable.
