# Translation Contributions

This guide explains how to add or update translated documentation while
keeping it accurate and aligned with the English source. It is for contributors
who translate project documentation or maintain an existing translation.

## Source and scope

English documentation is the reference when translations differ. `README_ES.md`
is the current Spanish entry point; other translated pages should be linked
from the corresponding English page or index so readers can find them.
Translate user-facing guides first, where a translation makes setup and safe
operation easier. Keep a translation focused on the same subject and scope as
its source page.

Do not translate code, commands, configuration keys, CLI flags, file paths,
protocol names, or literal output. Translate explanatory prose and comments
only when doing so does not change a copyable example. Keep links pointed at
the canonical source when no translated page exists.

## Translation workflow

1. Read the English source and its linked canonical references. Check current
   behavior in the implementation or CLI help when the source describes
   commands, configuration, compatibility, or security properties.
2. Preserve the source page's meaning, limits, heading hierarchy, warnings,
   and examples. Do not strengthen claims about security, anonymity, speed, or
   platform support.
3. Use the target language naturally and consistently. Keep product names,
   code identifiers, and established protocol terms unchanged; use the
   [glossary](../GLOSSARY.md) where it defines preferred terminology.
4. Add a reciprocal link between the source and translated page, with the
   language named in the link text. Update the relevant index when the page is
   part of a documentation collection.
5. Review the translation against the whole source, then check relative links,
   Markdown structure, and copyable commands. Run `git diff --check`.

## Keeping translations current

When changing an English page that has a translation, update both in the same
change when practical. If the translated page cannot be updated yet, mark it
clearly near the top with a link to the English source and a note that it may
be out of date. Remove that note after synchronization. Do not silently leave
changed commands, defaults, version requirements, or security limitations in
an older translation.

For a translation-only change, identify the source page and language in the
pull request. For a source change, mention which translations were updated or
remain out of date. Have a fluent reader review wording when available; machine
translation may be used as a draft, but the contributor remains responsible
for technical accuracy and natural phrasing.

## Project terminology and boundaries

Translations must preserve Espejismo's product positioning in
[`POSITIONING.md`](POSITIONING.md): the core tunnel does not impersonate other
protocols, and the project does not claim invisibility. Describe optional
WebSocket and HTTP/2 underlays as real transports, not camouflage. Keep the
small operating model and documented limitations intact.

If a term has no clear equivalent, retain the established technical term and
explain it briefly on first use rather than inventing a new product claim.
When the English source appears inconsistent with implementation or another
canonical reference, report the discrepancy instead of resolving it by
changing the translation's meaning.

## Review checklist

- The translation covers the same version and scope as its source, or is
  visibly marked as potentially out of date.
- Technical terms, numeric values, warnings, and product boundaries match the
  source and canonical references.
- Commands, code, configuration keys, and paths remain copyable and unchanged.
- Source and translation links work, and any relevant index is updated.
- The prose reads naturally in the target language and does not add unsupported
  claims.
