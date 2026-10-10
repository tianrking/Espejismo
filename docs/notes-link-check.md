# Internal Documentation Link Check

## Scope and method

Reviewed repository Markdown files for relative inline links and links to
heading anchors. The baseline scan covered 223 Markdown files, including archived
notes, because their links remain useful when consulting historical decisions.
The checker resolved file targets relative to each source file and checked
Markdown heading fragments. External URLs and reference-style definitions were
outside this focused pass.

## Findings and changes

The first pass found six broken relative file links, all in archived notes.
Their targets had moved into the top-level `docs/` tree while the archived
notes remained under `docs/archive/notes/`. Updated the links to point to the
current benchmark guide, research references, positioning, repository overview,
protocol specification, and deployment quickstart.

No product behavior or positioning changed. Keeping historical notes navigable
makes their supporting rationale available to maintainers and avoids stale
references being mistaken for missing current documentation.

## Validation and outcome

Re-ran the same checker over all 224 Markdown files, including this task note, after the edits. It found
zero missing relative file targets and zero missing Markdown heading anchors.
This is a documentation integrity check; no runtime tests or performance
measurements apply.
