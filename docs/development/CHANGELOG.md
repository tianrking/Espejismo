# Changelog maintenance

`CHANGELOG.md` is the user-facing record of notable changes. Keep entries concise,
plain-language, and focused on effects that matter to operators or users. It is
not a commit log: omit internal refactors, routine dependency updates, and work
that has no visible behavior, compatibility, security, or operational impact.

## While changes are unreleased

- Add new entries under `## Unreleased`, which stays above all released versions.
- Group entries under only the headings that apply: `Added`, `Changed`,
  `Fixed`, `Security`, and `Deprecated`. Omit empty headings.
- Describe the result and relevant scope, not implementation chronology. Mention
  configuration keys, CLI flags, compatibility effects, or limitations when a
  user needs them to understand or operate the change.
- Keep each entry to one focused bullet. Avoid repeating details already covered
  by a linked guide; link to that guide when it provides necessary instructions.
- Do not claim performance or reliability gains without recorded evidence. Link
  to benchmark or validation results when a claim depends on them.

## Preparing a release

- Confirm every entry under `Unreleased` still matches the shipped behavior and
  remove entries for reverted or superseded work.
- Move the unreleased entries under a heading using the release tag, such as
  `## v0.1.6`, and add a short summary only when it helps readers understand the
  scope of the release.
- Keep previous release sections intact. Do not rewrite historical entries to
  imply behavior that was not part of that release.
- Check links, configuration names, command names, and compatibility statements
  against the released artifacts and documentation.

See the root [CHANGELOG.md](../../CHANGELOG.md) for the project history.
