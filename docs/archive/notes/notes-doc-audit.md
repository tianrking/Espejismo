# Documentation link audit

## Scope and finding

Audited all 204 repository Markdown files for relative inline links and local
angle-bracket paths, including links to generated Markdown heading anchors.
The first pass checked 310 local links and found one broken fragment:
`docs/deployment/SYSTEMD.md` linked to
`docs/deployment/TUN.md#systemd-stop-hook-example`, while the target text was a
plain paragraph and did not generate that heading anchor.

## Change and rationale

Turned the target text in `docs/deployment/TUN.md` into the matching level-two
heading. This preserves the existing guidance and makes the cross-document
navigation target stable and usable.

## Verification and expected impact

Repeated the same full-repository scan after the edit: 204 Markdown files,
310 local links checked, zero missing paths or heading anchors. This is a
documentation-only correction; expected runtime performance change is 0%,
with no code or protocol behavior changed. No Cargo tests are applicable.

The audit covers local inline Markdown links, images, and relative/rooted
angle-bracket paths. External URL availability and reference-style link
definitions are outside this scan's scope; no reference-style definitions
were present in the repository Markdown files.
